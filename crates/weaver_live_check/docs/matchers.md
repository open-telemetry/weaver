# Matchers

Live-check compares a telemetry sample with a signal in your registry. Before it can do that it has to work out which signal the sample belongs to.

Some samples say so themselves. A metric has its name and an event has its `event_name`, and we look those up in the registry. Spans do not. A span name is free-form, so nothing in the sample tells us which span definition it was meant to be. Logs without an `event_name`, resources and instrumentation scopes are in the same position: a bag of attributes with no identifier on it.

A matcher gives live-check that identifier. You describe a signature you know your telemetry has, and you say which signal, or which attributes, a sample matching that signature should be compared with.

A matcher never changes the checks themselves. It only decides what a sample is compared with.

## The registry these examples use

Everything below runs against this one registry, so you can follow the outcomes as the matchers change.

```yaml
file_format: definition/2

attributes:
  - key: myapp.checkout.id
    type: string
    brief: Identifier of the checkout.
    stability: stable
    examples: ["3f9a1c"]
  - key: myapp.checkout.stage
    type: string
    brief: Stage the checkout reached.
    stability: stable
    examples: ["payment"]
  - key: myapp.cart.item_count
    type: int
    brief: Number of items in the cart.
    stability: stable
    examples: [3]
  - key: myapp.tenant.code
    type: string
    brief: Tenant the telemetry belongs to.
    stability: stable
    examples: ["acme-eu"]
  - key: myapp.request.id
    type: string
    brief: Identifier of the request.
    stability: development
    examples: ["7c1f"]

attribute_groups:
  - id: myapp.common
    visibility: public
    brief: The attributes we expect on telemetry from our own services.
    stability: development
    attributes:
      - ref: myapp.tenant.code
        requirement_level: required
      - ref: myapp.request.id
        requirement_level: recommended

spans:
  - type: myapp.checkout
    name:
      note: The constant `checkout`.
    brief: A checkout operation in the store front.
    stability: development
    kind: internal
    attributes:
      - ref: myapp.checkout.id
        requirement_level: required
      - ref: myapp.checkout.stage
        requirement_level: required
      - ref: myapp.cart.item_count
        requirement_level: recommended

metrics:
  - name: myapp.checkout.attempts
    brief: Number of checkout attempts.
    stability: development
    instrument: counter
    unit: "{attempt}"
    attributes:
      - ref: myapp.checkout.stage
        requirement_level: recommended
```

## A span with no matcher

Start with the problem. This span is exactly what the `myapp.checkout` definition describes:

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

With no matchers configured, live-check has nothing to compare it with:

```text
Span checkout payment `internal`
  none -> signal: no match
    myapp.checkout.id = 3f9a1c
        - [violation] Attribute 'myapp.checkout.id' does not exist in the registry.
    myapp.checkout.stage = payment
        - [violation] Attribute 'myapp.checkout.stage' does not exist in the registry.
```

Both attributes are declared in the registry, and both are on the span definition. But no signal was resolved, so there is no set of attributes to compare them against, and each one reports that it does not exist. This is the state a matcher fixes.

## Giving the span an identifier

The signature here is `myapp.checkout.id` being present. Any span with it is a checkout:

```toml
[[live-check.matchers]]
id = "match.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'
signal = "myapp.checkout"
```

The `id` is yours to choose to name the matcher in the output. It must be unique among all matchers. These examples use a `match.` prefix to make it clear what is a matcher and what is a signal.

The same span now:

```text
Span checkout payment `internal`
  match.checkout -> signal: myapp.checkout
    - [improvement] Recommended attribute 'myapp.cart.item_count' is not present.
    myapp.checkout.id = 3f9a1c
    myapp.checkout.stage = payment
```

The two violations are gone, because the span is being compared with the definition that declares those attributes. In their place is the finding you wanted: the span is missing a recommended attribute.

The line under the span name tells you what happened. `match.checkout -> signal: myapp.checkout` is the matcher id on the left and what it contributed on the right.

A span without the signature is left alone:

```json
{
  "span": {
    "name": "checkout refund",
    "kind": "internal",
    "attributes": [{ "name": "myapp.cart.item_count", "value": 3 }]
  }
}
```

```text
Span checkout refund `internal`
  none -> signal: no match
    myapp.cart.item_count = 3
        - [violation] Attribute 'myapp.cart.item_count' does not exist in the registry.
```

The span name looks right, but the name is not part of the signature. Write the signature around what your instrumentation actually guarantees.

## Attributes the signal does not declare

Real telemetry holds more than one signal's worth of attributes. Here the checkout span also sets the tenant:

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

With the matcher above, which names only the signal:

```text
Span checkout payment `internal`
  match.checkout -> signal: myapp.checkout
    - [improvement] Recommended attribute 'myapp.cart.item_count' is not present.
    - [improvement] Attribute 'myapp.tenant.code' is not in the matched signal or attribute groups.
    myapp.checkout.id = 3f9a1c
    myapp.checkout.stage = payment
    myapp.tenant.code = acme-eu
        - [violation] Attribute 'myapp.tenant.code' does not exist in the registry.
```

The attribute is reported twice over: once as unexpected on this signal, and once as unresolved, because it is not in the set the span was compared with.

That attribute is expected, though. It is declared in the registry, and it is in the `myapp.common` attribute group. Say so:

```toml
[[live-check.matchers]]
id = "match.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'
signal = "myapp.checkout"
attribute_groups = ["myapp.common"]
```

```text
Span checkout payment `internal`
  match.checkout -> signal: myapp.checkout
  match.checkout -> attribute_groups: myapp.common
    - [improvement] Recommended attribute 'myapp.cart.item_count' is not present.
    myapp.checkout.id = 3f9a1c
    myapp.checkout.stage = payment
    myapp.tenant.code = acme-eu
```

The span definition has not changed and neither has the signature. All the matcher says is that the attributes in `myapp.common` are permitted here too. The tenant is now compared against your refinement of it, so its type and stability are checked and any annotation-based policy you have written runs on it.

This matters beyond quietening a finding. Weaver promotes schema-driven practice, so the value is in comparing what you emit against the same definitions your documentation and generated code came from.

## Logs, which have no signal of their own

A log with an `event_name` matches an event in the registry by that name, in the normal way. A log without one has no identifier at all.

There is no log signal in semconv, so there is nothing to put in `signal`. What you can do is give the log a set of attributes to be compared with. In semconv a set of attributes is an attribute group.

It is common to want the same group on every log, and a matcher with no `when` does exactly that:

```toml
[[live-check.matchers]]
id = "match.log.common"
sample_type = "log"
attribute_groups = ["myapp.common"]
```

```json
{
  "log": {
    "event_name": "",
    "attributes": [{ "name": "myapp.request.id", "value": "7c1f" }]
  }
}
```

Without the matcher the attribute has nothing to compare against:

```text
Log
  none -> signal: no match
    myapp.request.id = 7c1f
        - [violation] Attribute 'myapp.request.id' does not exist in the registry.
```

With it:

```text
Log
  none -> signal: no match
  match.log.common -> attribute_groups: myapp.common
    myapp.request.id = 7c1f
        - [improvement] Attribute 'myapp.request.id' is not stable; stability = development.
```

The attribute now resolves to its definition, so it gets the checks that definition earns: its type, its stability, whether it is deprecated, and any annotation-based policy you have written.

`none -> signal: no match` is still on the line above. A log with no `event_name` names no signal, so that is expected rather than a gap, and live-check greys it rather than colouring it yellow.

A log that does name a declared event keeps that event, and the group is checked on top of it.

## Metrics, which already match

A metric resolves its own signal by name. You do not want a matcher to take that away, so leave `signal` out and add only the group:

```toml
[[live-check.matchers]]
id = "match.metric.common"
sample_type = "metric"
when = 'name.startsWith("myapp.")'
attribute_groups = ["myapp.common"]
```

```json
{
  "metric": {
    "name": "myapp.checkout.attempts",
    "instrument": "counter",
    "unit": "{attempt}",
    "data_points": [
      {
        "value": 1,
        "attributes": [
          { "name": "myapp.checkout.stage", "value": "payment" },
          { "name": "myapp.tenant.code", "value": "acme-eu" }
        ]
      }
    ]
  }
}
```

```text
Metric myapp.checkout.attempts `counter`, `{attempt}`
  none -> signal: myapp.checkout.attempts
  match.metric.common -> attribute_groups: myapp.common
    - [improvement] Metric 'myapp.checkout.attempts' is not stable; stability = development.
    Data point 1
        myapp.checkout.stage = payment
        myapp.tenant.code = acme-eu
```

`none -> signal:` means the metric's own name resolved the signal, not a matcher. The `when` on the name is what keeps this off the metrics you did not write: `http.client.request.duration` and everything else from your dependencies is left exactly as it was.

A metric's attributes live on its data points, so the group is checked against each point.

## Resources and instrumentation scopes

A resource is a list of attributes with no identifier and no signal that describes it, so an attribute group is the only thing it can be compared with. A matcher for a resource never has a `signal`.

It is tempting to compare a resource with an entity, but that would be wrong. Entities are pulled in by the signals: when a metric declares `entity_associations`, live-check takes the attributes that entity asks for out of the resource and checks them as part of that metric. One message holds many signals sharing one resource, so the resource holds the attributes every one of those entities needs, and probably more. It is a superset. Compared with any single entity, everything the other signals needed would be reported as unexpected.

```toml
[[live-check.matchers]]
id = "match.resource"
sample_type = "resource"
when = '"service.name" in attributes && attributes["service.name"].startsWith("myapp.")'
attribute_groups = ["myapp.common"]
```

The `when` keeps the matcher off the resources belonging to anything other than your own services.

A scope is in the same position, with one difference: it has an identifier. The name and version tell you which library produced the telemetry, so a scope matcher needs no signature.

```toml
[[live-check.matchers]]
id = "match.scope"
sample_type = "instrumentation_scope"
when = 'name.startsWith("myapp.")'
attribute_groups = ["myapp.common"]
```

The scope is often more useful inside other matchers than in one of its own. Any matcher can read `instrumentation_scope.name` and `instrumentation_scope.version`, so you can trust a signature only when it came from your own instrumentation:

```toml
[[live-check.matchers]]
id = "match.checkout"
sample_type = "span"
when = '''
instrumentation_scope.name.startsWith("myapp.")
  && "myapp.checkout.id" in attributes
'''
signal = "myapp.checkout"
```

A span with the same attributes from somewhere else no longer matches, and its `match_info` says nothing applied.

> **Note**: `instrumentation_scope` is only bound for OTLP input today. On a JSON file or stdin it is unbound and the expression errors on every sample.

## Where matchers go

Matchers describe the telemetry you emit, not the schema you define, so they belong in `.weaver.toml` and not in the registry. They are an array of tables, evaluated in the order you write them.

```toml
[[live-check.matchers]]
id = "match.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'
signal = "myapp.checkout"
attribute_groups = ["myapp.common"]
```

| Field              | Required | Description                                                                                                                                            |
| ------------------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `id`               | Yes      | Names the matcher in findings, statistics and coverage.                                                                                                |
| `sample_type`      | Yes      | The kind of sample this matcher looks at. One of `span`, `span_event`, `span_link`, `log`, `metric`, `resource`, `instrumentation_scope` or `profile`. |
| `when`             | No       | The matcher expression, in CEL. It has to be true for the matcher to apply. Leave it out and the matcher applies to every sample of this type.         |
| `signal`           | No       | The one signal the sample is compared with. Leave it out to keep the natural match.                                                                    |
| `attribute_groups` | No | Attribute groups whose attributes are *permitted* on the sample, in priority order. Their definitions are used for the attribute checks, but an attribute missing from the sample is not reported. |
| `strict_attribute_groups` | No | Attribute groups whose requirement levels are *enforced*, so an attribute missing from the sample is reported. |

Matchers are a v2 feature. Configuring one against a v1 registry stops the run at startup.

### What `signal` can name

References are plain ids, and `sample_type` decides what `signal` means.

| `sample_type`           | What `signal` names              | The natural match, if `signal` is left out |
| ----------------------- | -------------------------------- | ------------------------------------------ |
| `span`                  | The `type` of a span             | None                                       |
| `span_event`            | The `name` of an event           | None                                       |
| `span_link`             | Nothing, `signal` is not allowed | None                                       |
| `log`                   | The `name` of an event           | The event, by `event_name`                 |
| `metric`                | The `name` of a metric           | The metric, by name                        |
| `resource`              | Nothing, `signal` is not allowed | None                                       |
| `instrumentation_scope` | Nothing, `signal` is not allowed | None                                       |
| `profile`               | Nothing, `signal` is not allowed | None                                       |

The id is looked up in your registry at startup, so a name that is not there stops the run there rather than halfway through a stream.

An attribute group never goes in `signal`. A group adds to the comparison rather than replacing it, so it always goes in one of the two group lists.

A group named in `strict_attribute_groups` by any matcher that applied is strict, whichever matcher mentioned it first.

## The expression

`when` is written in [CEL](https://cel.dev), the Common Expression Language. CEL was designed for this job: it is not Turing complete, and an expression cannot loop or reach outside the sample it is given. Every expression is compiled once at startup and then run against each sample.

The expression looks at one sample and comes out true or false. These are the variables it is given:

| Selector                                                 | Where you can use it                            | What you get                                                                                                                                             |
| -------------------------------------------------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `attributes["key"]`                                      | Any sample                                      | The attributes on the sample, as a map. On a metric this is the attributes every data point agrees on; a key they hold different values for is left out. |
| `resource.attributes["key"]`                             | Any signal sample                               | The attributes on the resource the sample arrived with.                                                                                                  |
| `instrumentation_scope.name`, `.version`, `.schema_url`  | Any signal sample                               | The instrumentation scope that produced the sample.                                                                                                      |
| `instrumentation_scope.attributes["key"]`                | Any signal sample                               | The attributes on that scope.                                                                                                                            |
| `name`                                                   | Span, span event, metric, instrumentation scope | The span name, event name, metric name or scope name.                                                                                                    |
| `kind`                                                   | Span                                            | One of `client`, `server`, `internal`, `producer` or `consumer`.                                                                                         |
| `status.code`, `status.message`                          | Span                                            | The outcome of the span. The code is one of `unset`, `ok` or `error`, and a span with no status is `unset`.                                              |
| `unit`, `instrument`                                     | Metric                                          | The unit and the instrument of the metric.                                                                                                               |
| `event_name`, `severity_text`, `severity_number`, `body` | Log                                             | The fields on the log record. All but `event_name` are optional, and one the record omits is unbound, so reading it errors.                     |

CEL brings the operators you would expect, `==`, `!=`, `&&`, `||`, `!` and brackets, along with the string methods `matches`, `startsWith`, `endsWith` and `contains`, and the macros `has`, `exists`, `exists_one`, `all`, `map` and `filter`.

| Expression                               | What it does                                                                                                                                                                |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `"key" in attributes`                    | True if the attribute is on the sample.                                                                                                                                     |
| `attributes["key"] == "value"`           | True if the value is the one you give.                                                                                                                                      |
| `attributes["key"] in ["a", "b"]`        | True if the value is one of the ones you list.                                                                                                                              |
| `attributes["key"].startsWith("myapp.")` | Also `endsWith` and `contains`.                                                                                                                                             |
| `attributes["key"].matches("regex")`     | True if the value is a string and the regular expression matches it. The pattern is compiled on every sample, so prefer `in` or `startsWith` where they say the same thing. |

### Guarding a read

Reading an attribute absent from the sample is an error in CEL, not an empty value. So every value you read needs an `in` test on the same key:

```cel
"myapp.checkout.stage" in attributes
  && attributes["myapp.checkout.stage"] in ["cart", "payment", "confirm"]
```

An `&&` absorbs that error while the other side is false, whichever side the test is on.

Without the guard the matcher errors on every sample missing the key, applies to nothing, and the run still finishes. The coverage block reports the count and the first message:

```text
Matcher coverage
  - match.checkout: 0 sample(s), 1 error(s): The expression
    `attributes["myapp.checkout.stage"] == "payment"` failed to evaluate:
    No such key: myapp.checkout.stage
```

and two warnings land in the diagnostic report at the end of the run:

```text
  ⚠ Matcher `match.checkout` errored on 1 sample(s). First error: ...

  ⚠ Matcher `match.checkout` applied to no samples.
```

## Resolution order

For each sample:

1. First the natural match, so a metric by its name and a log by its `event_name`.
2. Then every matcher whose `sample_type` and `when` both pass.
3. The first matcher with a `signal` sets it, overriding the natural match. If another matcher also has a `signal` it is ignored and named in the sample's `match_info`.
4. The attribute groups from every matcher that applied are added in the order they are written, first mention winning, strict before permitted within one matcher.
5. Only the strict groups have their requirement levels enforced.
6. The sample and its attributes are compared with the signal and the groups together.

This is why a matcher that only adds attributes leaves `signal` out. If it named one it would take a typed sample away from the signal it already matched.

## What a sample was checked against

Every sample's result holds a `match_info`: the signal, the matcher whose `signal` won, the attribute groups, and one entry per matcher that applied. The ansi output puts one dimmed line under the sample for each thing a matcher contributed.

This is a fuller set of matchers than the ones above, to show what several of them look like together:

```text
Span checkout `server`
  match.checkout.by-name -> signal: myapp.checkout
  match.checkout.by-name -> attribute_groups: myapp.session, myapp.customer

Span cart `internal`
  match.cart.by-attribute -> signal: myapp.cart
  match.span.by-scope -> attribute_groups: myapp.customer
  match.cart.conflict -> signal: myapp.checkout (conflict, ignored)

Span unknown-op `internal`
  none -> signal: no match
```

`none` means the sample's own name resolved the signal, or that nothing set one. `no match` is yellow on a sample that should resolve a signal and has not: a span, a span event, a metric, or a log with an `event_name`. It is grey on a resource, a scope, a span link, a profile and a log with no `event_name`, none of which name a signal. `(conflict, ignored)` is red.

`match_info` is not a finding, so it does not reach `finding_filters`, `fail_on` or the emitted OTLP logs.

## Related configuration

```toml
[live-check]
search_all_attributes = true
```

With this set, live-check also searches the base attribute definitions in your registry and its dependencies. An attribute found that way names the `schema_url` that declares it, which is a clue that your registry could reference or import it.

Without it a v2 registry compares an attribute against the signal and attribute groups its match holds, and nothing else. That is what the first example on this page shows: no match, so nothing to compare against. The plain attribute-name inputs, `--input-format text` and a JSON file of bare attributes, have no match at all and need this setting. A v1 registry always searches every attribute it holds.

## Diagnostics

Everything a matcher can get wrong is reported, either at startup or in the matcher coverage block at the end of a run.

| What happened                                                  | When you hear about it                      |
| -------------------------------------------------------------- | ------------------------------------------- |
| The expression does not parse                                  | Startup, run stops                          |
| The expression reads a variable that sample type does not have | Startup, run stops                          |
| A literal `matches` pattern is not a valid regex               | Startup, run stops                          |
| `signal` or an attribute group name is not in the registry     | Startup, run stops                          |
| A matcher is configured against a v1 registry                  | Startup, run stops                          |
| The expression errors while running, e.g. an unguarded read    | Warning, with a count and the first message |
| A matcher applied to no samples                                | Warning                                     |

The coverage block lists what each matcher matched, so you can see which ones are earning their place:

```text
Matcher coverage
  - match.checkout: 1 sample(s)
  - match.legacy: 0 sample(s)
```

## New findings

| Finding                | Level       | When it is raised                                                                                                                                                                                                                                                                                      |
| ---------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `unexpected_attribute` | Improvement | A sample has an attribute that is not in the comparison set. A metric or a log has one as soon as its name resolves, so this fires with no matchers configured; a span needs a matcher. When a matcher sets `signal` the comparison set comes from that signal, not from the one the name resolved to. |
| `kind_mismatch`        | Violation   | A matched span has a different `kind` to the one on the span signal. Comparing spans is what makes this check possible at all.                                                                                                                                                                         |
