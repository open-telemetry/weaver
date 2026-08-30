# Matching

The point of live-check is to compare incoming telemetry with a defined schema and to produce information about the mismatch between them. In order to make a comparison, an element of the telemetry, a sample, needs to be matched with an element of the schema, a signal.

Some samples are easily matched to signals because the signal contains an identifier that is included in the sample. Metrics and Events are in this category.

Within signals are attributes, an attribute name is an identifier and so these easily match.

A large section of telemetry today does not land in the easily matched category though. Spans, for example, do not have an identifier. Nor do logs. But both these signals contain attributes that we would like to match on.

The current implementation of live-check was written for v1 semconv and single registries (no dependencies on other registries). In v2 semconv the definition of the registries is more correct and exact with a higher focus on being signal-based which makes it now unnatural to use the simplistic "best effort" approach that live-check took for v1. This is particularly problematic around the checking of attributes against untyped telemetry samples: log and span.

Here's a table of the current Samples and Signals and description of how they are matched:

| Sample                                                | Signal             | Identifier             | How they are matched                                                                                                                                                                                                                                                |
| ----------------------------------------------------- | ------------------ | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Metric                                                | Metric             | Metric name            | Look up the name in the registry. If there is nothing, and no matcher names a signal for it, we get a `missing_metric` finding.                                                                                                                                       |
| Data point (number, histogram, exponential histogram) | Metric (inherited) | None                   | It uses the metric that its parent matched, so the attributes on the data point are compared with the attributes on that metric.                                                                                                                                    |
| Exemplar                                              | Metric (inherited) | None                   | The same as data points. It uses the metric that its parent matched.                                                                                                                                                                                                |
| Log                                                   | Event              | `event_name`           | Look up the event name in the registry. If there is nothing, and no matcher names a signal for it, we get a `missing_event` finding. A log with an empty `event_name` is not matched at all, so its attributes are only compared against the registry as a whole. |
| Span                                                  | Span               | None                   | Not matched. The span name is free-form so no span signal is chosen, and every attribute is matched on its own key against the whole registry.                                                                                                                      |
| Span event                                            | Event              | `name`                 | Not matched. Unlike a log, the name is never looked up as an event. The attributes are matched on their own keys.                                                                                                                                                   |
| Span link                                             | None               | None                   | Not matched. The attributes are matched on their own keys.                                                                                                                                                                                                          |
| Resource                                              | None               | None                   | Not matched on its own, so the attributes are matched on their own keys. When a metric or event declares `entity_associations` we look up those entities and check the attributes they ask for against the resource, but that check belongs to the metric or event. |
| Instrumentation scope                                 | None               | Scope name and version | Not matched. There is no scope signal in semconv. The attributes are matched on their own keys.                                                                                                                                                                     |
| Profile                                               | None               | None                   | Not matched. There is no profile signal in semconv. The attributes are matched on their own keys.                                                                                                                                                                   |

So, how can we attempt to match an untyped telemetry sample?

## Span

Where there is no identifier, we can use a Matcher to detect a signature in the sample to take the place of that identifier. For Span that could be the presence of an attribute or attributes, or particular values in particular attributes, or perhaps those values match a regular expression. Any span sample that matches this signature can then be compared against a declared span signal in the registry.

Here is the span signal in the registry:

```yaml
file_format: definition/2
spans:
  - type: myapp.checkout
    brief: A checkout operation in the store front
    stability: development
    kind: internal
    attributes:
      - ref: myapp.checkout.id
        requirement_level: required
      - ref: myapp.checkout.stage
        requirement_level: required
      - ref: myapp.cart.item_count
        requirement_level: recommended
```

And here is the matcher that gives the signal an identifier. The signature is `myapp.checkout.id` being present, together with `myapp.checkout.stage` holding one of the values we expect:

```toml
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
when = '''
"myapp.checkout.id" in attributes
  && "myapp.checkout.stage" in attributes
  && attributes["myapp.checkout.stage"].matches("^(cart|payment|confirm)$")
'''
signal = "myapp.checkout"
```

This sample matches. It is compared against the `myapp.checkout` span, so we get a `recommended_attribute_not_present` finding for the missing `myapp.cart.item_count`:

```json
{
  "span": {
    "name": "checkout payment",
    "kind": "internal",
    "attributes": [
      { "name": "myapp.checkout.id", "value": "3f9a1c" },
      { "name": "myapp.checkout.stage", "value": "payment" }
    ]
  }
}
```

This one does not match because the signature attribute is not there. The span name looks right, but the name is not part of the signature:

```json
{
  "span": {
    "name": "checkout payment",
    "kind": "internal",
    "attributes": [
      { "name": "myapp.cart.item_count", "value": 3 }
    ]
  }
}
```

This one does not match either. It has the signature attribute, but `myapp.checkout.stage` does not match the regex:

```json
{
  "span": {
    "name": "checkout refund",
    "kind": "internal",
    "attributes": [
      { "name": "myapp.checkout.id", "value": "3f9a1c" },
      { "name": "myapp.checkout.stage", "value": "refund" }
    ]
  }
}
```

## Log

Logs do not have a defined signal in the semconv schema. However we still want to be able to check the attributes delivered within the log. We want to know that we have defined the attribute, that the incoming type of the value is correct, that it's not deprecated, it's stable and perform any additional checks in policies that may use annotations specified on the attribute.

To do these checks we need to provide a Matcher that detects the signature but returns a set of attributes for comparison since there is no log signal.

In semconv a set of attributes is an Attribute Group. The group lists references to attribute definitions and allows for refinements such as adding annotations.

It's common to want to match many logs to the same attribute group, maybe all logs even. This example does exactly that:

The attribute group, with the annotation on the refinement of `myapp.tenant.code`:

```yaml
file_format: definition/2
attribute_groups:
  - id: myapp.common
    brief: The attributes we expect on the telemetry from our own services
    stability: development
    attributes:
      - ref: myapp.tenant.code
        requirement_level: required
        annotations:
          live_check:
            case: upper
      - ref: myapp.request.id
        requirement_level: recommended
```

The matcher. There is no `when`, so it applies to every log we receive:

```toml
[[live-check.matchers]]
id = "myapp.common.log"
sample_type = "log"
attribute_groups = ["myapp.common"]
```

A log that names an event we have declared still matches that event in the normal way, and the group is checked on top of it.

The annotation does nothing on its own. It is there for a policy to read, and the policy sees it on `input.registry_attribute`:

```rego
package live_check_advice

import rego.v1

deny contains make_advice(advice_type, advice_level, advice_context, message) if {
	input.sample.attribute
	input.registry_attribute.annotations.live_check.case == "upper"
	value := input.sample.attribute.value
	is_string(value)
	value != upper(value)
	advice_type := "case_mismatch"
	advice_level := "violation"
	advice_context := {"attribute_key": input.sample.attribute.name}
	message := sprintf("Attribute '%s' must have an upper case value", [input.sample.attribute.name])
}
```

So this log gets a `case_mismatch` violation on `myapp.tenant.code`, and nothing on `myapp.request.id`:

```json
{
  "log": {
    "event_name": "myapp.order.placed",
    "attributes": [
      { "name": "myapp.tenant.code", "value": "acme-eu" },
      { "name": "myapp.request.id", "value": "7c1f" }
    ]
  }
}
```

The same finding appears on a log with no event name at all. The event match is what we lose without an identifier, the attributes are still checked.

## Additional attributes

Often additional attributes are included in the incoming telemetry samples that are not defined on the signals in the schema. In the next version of live-check these attributes will result in a Finding e.g. "Unexpected Attribute"

But what if you want to allow this?

If enabled, live-check will search all base definitions of attributes in your registry and its dependencies to improve on the Finding: "Unexpected Attribute: `myapp.special.thing` found in schema https://example.com/myschema/1.0.0"

However, if this additional attribute IS expected I do not want to have an Unexpected Attribute finding and, more importantly, I want to compare it against my defined refinements in my registry. This allows for annotation based checks and to match documentation and codegen. Weaver promotes schema-driven practices, so it's vital that the documentation and code generated from the schema is accurately comparable by live-check to confirm the implementation.

Matching with additional attributes is easy through a matcher expression. Simply list the attribute groups in `attribute_groups`:

```toml
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
when = '''
"myapp.checkout.id" in attributes
  && "myapp.checkout.stage" in attributes
  && attributes["myapp.checkout.stage"].matches("^(cart|payment|confirm)$")
'''
signal = "myapp.checkout"
attribute_groups = ["myapp.common"]
```

The span signal has not changed and neither has the signature. All we have said is that the attributes in `myapp.common` are expected on this span too, so this sample is now fully accounted for:

```json
{
  "span": {
    "name": "checkout payment",
    "kind": "internal",
    "attributes": [
      { "name": "myapp.checkout.id", "value": "3f9a1c" },
      { "name": "myapp.checkout.stage", "value": "payment" },
      { "name": "myapp.tenant.code", "value": "acme-eu" }
    ]
  }
}
```

Without the `attribute_groups` entry `myapp.tenant.code` would be an `unexpected_attribute`. With it, the attribute is compared against my refinement of it, so instead I get the `case_mismatch` violation that I wanted, which is the same finding I would get for it on a log.

This is also possible on typed signals, e.g. Metrics. Here you omit `signal` (otherwise you would override the natural match) and just include `attribute_groups`.

```toml
[[live-check.matchers]]
id = "myapp.common.metric"
sample_type = "metric"
when = 'name.startsWith("myapp.")'
attribute_groups = ["myapp.common"]
```

The test is on the metric name, so this only reaches our own metrics. Every one of them keeps the natural match to its declared metric signal, and gets the attributes in `myapp.common` on top. The metrics from the libraries we depend on, `http.client.request.duration` and the like, are left exactly as they were.

The name is all we need here. Where the name is not enough, the same expression can go on to test the attributes, the unit, or anything else the sample carries.

## Resource

A resource on its own is a list of attributes. There is no identifier in it, and there is no signal in the schema that describes a whole resource, so the only thing we can compare it with is an attribute group.

It's tempting to compare a resource with an entity, but that would be wrong. Entities are pulled in by the signals. A metric can declare `entity_associations`, and when it does we take the attributes that the entity asks for out of the resource and check them as part of that metric. The finding belongs to the metric, not to the resource.

One incoming message can carry many metrics and they all share the same resource. That resource holds the attributes for every entity that every one of those signals asks for, and it may well hold more besides. It's a superset. If we compared it with any one entity we would report all of the attributes that the other signals needed as unexpected.

So a matcher for a resource has no `signal` at all. It only adds `attribute_groups`. The entity checks carry on as they are, driven by the signals that declare them.

The `when` here keeps the matcher off the resources that belong to anything other than our own services:

```toml
[[live-check.matchers]]
id = "myapp.resource"
sample_type = "resource"
when = '"service.name" in attributes && attributes["service.name"].startsWith("myapp.")'
attribute_groups = ["myapp.resource"]
```

## Instrumentation Scope

A scope is in much the same position as a resource. There is no scope signal in semconv, so an attribute group is the only thing we can compare its attributes with. It is shared by every signal in its part of the message too, so it is a superset in the same way a resource is.

What a scope does have, and a resource does not, is an identifier. The name and version tell us which library produced the telemetry, so a matcher for a scope needs no signature at all. A `when` of `name == "io.opentelemetry.jdbc"`, or of `name.startsWith("myapp.")` for a family of them, is enough to pick one out.

```toml
[[live-check.matchers]]
id = "myapp.scope"
sample_type = "instrumentation_scope"
when = 'name.startsWith("myapp.")'
attribute_groups = ["myapp.scope"]
```

The scope is often more useful in other matchers than in one of its own. Any matcher can read `instrumentation_scope.name` and `instrumentation_scope.version` in its expression, so you can write a matcher that only applies to the spans from one library, or treat the telemetry from your own instrumentation differently to the telemetry from a framework you have no control over.

This is the checkout matcher from earlier with one more clause on the front of it. The signature is now only trusted when the span came from our own instrumentation:

```toml
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
when = '''
instrumentation_scope.name.startsWith("myapp.")
  && "myapp.checkout.id" in attributes
  && "myapp.checkout.stage" in attributes
  && attributes["myapp.checkout.stage"].matches("^(cart|payment|confirm)$")
'''
signal = "myapp.checkout"
```

A span that carries the same attributes but comes from somewhere else no longer matches `myapp.checkout`. It falls back to the attribute by attribute check and an `unmatched_sample` finding.

A scope also carries a `schema_url`, which is the schema the telemetry claims to follow. We ingest it today and hand it to policies, but we do nothing else with it. It is worth comparing it with the `schema_url` of the registry we are checking against, because if the two disagree then the samples were built against a different version of the schema and some of the findings that follow are explained by that. A finding at information level when they differ would tell you so straight away.

---
# Matcher specification

A Matcher gives live-check an identifier for the samples that do not have one. It can also add attributes to the set that a sample is compared with. A matcher never changes the checks themselves, it only decides what a sample is compared with.

## Where matchers are defined

Matchers describe the telemetry you emit, not the schema you define, so they belong in `.weaver.toml` and not in the registry. They are an array of tables and they are evaluated in the order you write them:

```toml
[[live-check.matchers]]
id = "myapp.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'
signal = "myapp.checkout"
attribute_groups = ["myapp.common"]
```

| Field              | Required | Description                                                                                                                                            |
| ------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`               | Yes      | Names the matcher in findings, statistics and coverage.                                                                                                |
| `sample_type`      | Yes      | The kind of sample this matcher looks at. One of `span`, `span_event`, `span_link`, `log`, `metric`, `resource`, `instrumentation_scope` or `profile`. |
| `when`             | No       | The matcher expression, written in CEL. It has to be true for the matcher to apply. Leave it out and the matcher applies to every sample of this type. |
| `signal`           | No       | The one signal the sample is compared with. Leave it out to keep the natural match.                                                                    |
| `attribute_groups` | No       | Attribute groups, in priority order, to add to the comparison, on top of whatever `signal` brought in.                                                 |

## References

References are plain ids. What `signal` names is decided by the `sample_type`.

| `sample_type` | What `signal` names | The natural match, if `signal` is left out |
|---|---|---|
| `span` | The `type` of a span | None |
| `span_event` | The `name` of an event | None |
| `span_link` | Nothing, `signal` is not allowed | None |
| `log` | The `name` of an event | The event, by `event_name` |
| `metric` | The `name` of a metric | The metric, by name |
| `resource` | Nothing, `signal` is not allowed | None |
| `instrumentation_scope` | Nothing, `signal` is not allowed | None |
| `profile` | Nothing, `signal` is not allowed | None |

We look the id up in your registry. If it is not there live-check stops with an error at startup, rather than halfway through a stream.

An attribute group never goes in `signal`. A group adds to the comparison rather than replacing it, so it always goes in `attribute_groups`. The attributes in it are compared just as if the signal had declared them, with the same requirement levels and the same refinements. So a matcher can add its group to every log, and any log that names a declared event still keeps that event.

## The matcher expression

The expression is written in [CEL](https://cel.dev), the Common Expression Language. We do not need a language of our own for this. CEL was designed for exactly this job, it is not Turing complete, an expression cannot loop or reach outside the sample it is given, and the [`cel`](https://crates.io/crates/cel) crate gives us the whole language for the cost of one dependency. Every expression is compiled once when live-check starts and then run against each sample.

The expression looks at one sample and comes out true or false. These are the variables it is given:

| Selector                                                                                          | Where you can use it                            | What you get                                                                                                |
| ------------------------------------------------------------------------------------------------- | ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| `attributes["key"]`                                                                               | Any sample                                      | The attributes on the sample, as a map. On a metric this is the attributes every data point agrees on; a key they hold different values for is left out. |
| `resource.attributes["key"]`                                                                      | Any signal sample                               | The attributes on the resource the sample arrived with.                                                     |
| `instrumentation_scope.name`, `instrumentation_scope.version`, `instrumentation_scope.schema_url` | Any signal sample                               | The instrumentation scope that produced the sample.                                                         |
| `instrumentation_scope.attributes["key"]`                                                         | Any signal sample                               | The attributes on that scope.                                                                               |
| `name`                                                                                            | Span, span event, metric, instrumentation scope | The span name, event name, metric name or scope name.                                                       |
| `kind`                                                                                            | Span                                            | One of `client`, `server`, `internal`, `producer` or `consumer`.                                            |
| `status.code`, `status.message`                                                                   | Span                                            | The outcome of the span. The code is one of `unset`, `ok` or `error`, and a span with no status is `unset`. |
| `unit`, `instrument`                                                                              | Metric                                          | The unit and the instrument of the metric.                                                                  |
| `event_name`, `severity_text`, `severity_number`, `body`                                          | Log                                             | The fields on the log record. All but `event_name` are optional, and one the record does not carry is unbound, so reading it errors. |

CEL brings the operators you would expect, `==`, `!=`, `&&`, `||`, `!` and brackets, along with the string methods `matches`, `startsWith`, `endsWith` and `contains`, and the macros `has`, `exists`, `exists_one`, `all`, `map` and `filter`. Everything in a matcher is plain CEL, so anything you already know about the language holds here.

| Expression | What it does |
|---|---|
| `"key" in attributes` | True if the attribute is on the sample. |
| `attributes["key"] == "value"` | True if the value is the one you give. |
| `attributes["key"] in ["a", "b"]` | True if the value is one of the ones you list. |
| `attributes["key"].matches("regex")` | True if the value is a string and the regular expression matches it. |
| `attributes["key"].startsWith("myapp.")` | Also `endsWith` and `contains`. |

## Linting

Reading an attribute the sample does not carry is an error in CEL and not an empty value, so every value you read needs an `in` test on the same key:

```cel
"myapp.checkout.stage" in attributes
  && attributes["myapp.checkout.stage"].matches("^(cart|payment|confirm)$")
```

An `&&` absorbs that error while the other side is false, whichever side the test is on.

We compile and lint every expression at startup, so a matcher that does not parse, or that reads a variable we do not have for that sample type, stops us there. Nothing else is checked until the expression runs: an unguarded read of an absent key, an unknown function, and the wrong number of arguments are all runtime errors. We log the first one once as a diagnostic against the matcher, with a count, not as a finding.

## Resolution order

For each sample:

1. First we take the natural match, so a metric by its name and a log by its `event_name`.
2. Then we apply every matcher whose `sample_type` and `when` both pass.
3. The first matcher with a `signal` sets it, and it overrides the natural match. If another matcher also has a `signal` we ignore it and report `matcher_conflict` once at information level.
4. The `attribute_groups` from all of the applied matchers are added in the order they're defined in the toml, first definition wins.
5. The sample and its attributes are then compared with the primary signal and the secondary groups together.

This is why a matcher that only adds attributes leaves `signal` out. If it declared one it would take a typed sample away from the signal it already matched.

## New Findings

| Finding                | Level       | When we raise it                                                                                                                                                |
| ---------------------- | ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unmatched_sample`     | Information | An untyped sample matched no matcher. Its attributes are still checked one at a time against the registry if `search_all_attributes` is set.                    |
| `unexpected_attribute` | Improvement | A sample has an attribute that is not in the comparison set. A metric or a log has one as soon as its name resolves, so this fires with no matchers configured; a span needs a matcher. When a matcher sets `signal` the comparison set comes from that signal, not from the one the name resolved to. |
| `matcher_conflict`     | Information | More than one of the applied matchers had a `signal`.                                                                                                           |
| `kind_mismatch`        | Violation   | A matched span has a different `kind` to the one on the span signal.<br><br>(Since we can now compare spans this unlocks the ability to check the span's kind.) |

We count matched samples against the `id` of the matcher, so coverage can tell you which matchers fired and which never did.

## Related configuration

```toml
[live-check]
search_all_attributes = true
```

With this set we search the base attribute definitions in your registry and its dependencies. Attributes found through this route will have a finding including the `schema_url`. This could be a clue to help the author to improve their registry through a reference or import clause.

---
# Bonus: conditionally required attributes

A `conditionally_required` requirement level states its condition as an English sentence. A person can read it, but live-check cannot. So live-check treats the attribute as optional and checks nothing.

Some conditions are only about the sample itself. This one, from `aws.ecs`, is:

```yaml
- ref: aws.ecs.task.id
  requirement_level:
    conditionally_required: If and only if `task.arn` is populated.
```

In CEL it is one clause:

```cel
"aws.ecs.task.arn" in attributes
```

If that is true, the attribute is required for this sample, and a missing `aws.ecs.task.id` gives a `required_attribute_not_present` finding. If it is false, the attribute stays optional and live-check reports nothing.

The `when` condition could be an optional part of the schema, like so:

```yaml
- ref: aws.ecs.task.id
  requirement_level:
    conditionally_required: If and only if `task.arn` is populated.
    when: '"aws.ecs.task.arn" in attributes'
```

The expression language is the same as a matcher. Same variables, same compile at startup, same lint. 

You cannot express many of the conditions as a `when`. `If available.` and `If applicable.` are the two most common in semconv, and neither one is about anything live-check can see. Those attributes stay as they are.

These are the kind that are worth writing:

| Sentence | Expression |
|---|---|
| If and only if `task.arn` is populated. | `"aws.ecs.task.arn" in attributes` |
| If `server.address` is set. | `"server.address" in attributes` |
| if and only if k8s.hpa.metric.type is ContainerResource | `"k8s.hpa.metric.type" in attributes && attributes["k8s.hpa.metric.type"] == "ContainerResource"` |
| Required if `exception.type` is not set, recommended otherwise. | `!("exception.type" in attributes)` |

Some conditions are about the attribute they apply to. `server.port` is `conditionally_required: If not default (443).` When the port is missing there is no value to compare with 443. You cannot express a condition like that, so it keeps the sentence alone.

This is a bonus and not part of the matcher work. It is here because it uses the same expression engine, and because it would add a check where there is none today.

## How many could be expressed

I used an agent to scan every `conditionally_required` in the semconv model directory, at v1.44.0. There are 290 of them. 60 sit in deprecated files. The counts below leave those out.

| Condition | Count | Share |
| --- | --- | --- |
| A `when` today | 46 | 20% |
| About the outcome of a metric or an event, which has no status | 36 | 16% |
| One clause you can express, one you cannot | 13 | 6% |
| About the value of the attribute it applies to | 24 | 10% |
| On an attribute group that spans and metrics share | 4 | 2% |
| Not about anything live-check can see | 107 | 46% |

Three things come out of the scan.

The largest group is about failure. 47 conditions set `error.type`, or something like it, when the operation failed. `status.code == "error"` expresses that, and it is the only way to express it: a `when` of `"error.type" in attributes` is true exactly when the attribute is there, so it can never report the attribute missing.

The status only helps on a span. 7 of those 47 are on a span, and they move. 36 are on a metric or an event, which have no status and no other sign that the operation failed, so those conditions cannot be written at all. The last 4 sit on attribute groups that spans and metrics share, such as `attributes.http.common`. An expression there has to hold for every sample the group lands on, and `status` is not bound on a metric, so the expression would error on half of them. Those 4 need an answer to the shared group question before they can move.

The `server.port` conditions show what the expressions are worth. Seven different sentences say some version of "if `server.address` is set", on 13 refinements of `server.port`. The sentences all differ. The expression does not.

Writing the `when` is a job for a person and not a script. The ECS sentence says `task.arn` where the key is `aws.ecs.task.arn`, because the sentence uses the name that reads well in the group it sits in. A tool cannot always tell which attribute a sentence means.
