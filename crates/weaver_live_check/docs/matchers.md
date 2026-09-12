# Matchers

Live-check compares a telemetry sample with a signal in your registry. Before it can do that it has to work out which signal the sample belongs to.

Some samples carry their own identifier. A metric has its name and an event has its `event_name`, and live-check looks those up in the registry. A span does not. Its name is free-form, so nothing in the sample says which span definition it belongs to. A log without an `event_name`, a resource and an instrumentation scope are in the same position: a set of attributes with no identifier.

A matcher supplies that identifier. You describe a signature that your telemetry is known to have. Then you say which signal, or which attributes, to compare a matching sample with.

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

Both attributes are declared in the registry, and both are on the span definition. But no signal was resolved, so there is nothing to compare them with, and each one is reported as unknown. A matcher fixes this.

## Giving the span an identifier

The signature here is `myapp.checkout.id` being present. Any span with it is a checkout:

```toml
[[live-check.matchers]]
id = "match.checkout"
sample_type = "span"
when = '"myapp.checkout.id" in attributes'
signal = "myapp.checkout"
```

The `id` names the matcher in the output. Choose any name, as long as it is unique among the matchers. These examples use a `match.` prefix to tell matchers apart from signals.

The same span now:

```text
Span checkout payment `internal`
  match.checkout -> signal: myapp.checkout
    - [improvement] Recommended attribute 'myapp.cart.item_count' is not present.
    myapp.checkout.id = 3f9a1c
    myapp.checkout.stage = payment
```

The two violations are gone, because the span is now compared with the definition that declares those attributes. In their place is the finding you wanted: the span is missing a recommended attribute.

The line under the span name shows what happened. The matcher id is on the left and what it contributed is on the right.

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

The span name looks right, but the name is not part of the signature. Base the signature on what your instrumentation guarantees.

## Attributes the signal does not declare

Real telemetry carries attributes from more than one source. Here the checkout span also sets the tenant:

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

The attribute is reported twice: once as unexpected on this signal, and once as unknown, because it is not in the set the span was compared with.

But that attribute is expected. It is declared in the registry, and it is in the `myapp.common` attribute group. Say so:

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

The span definition and the signature are unchanged. The matcher only adds that the attributes in `myapp.common` are permitted here too. The tenant is now compared with its definition in that group, so its type and stability are checked, and any annotation-based policy you have written runs on it.

The point is not to silence a finding. The attribute is now checked against the same definition that your documentation and generated code come from.

## Logs, which have no signal of their own

A log with an `event_name` matches the event of that name in the registry. A log without one has no identifier at all.

Semconv has no log signal, so there is nothing to put in `signal`. Instead, give the log a set of attributes to compare with. In semconv, a set of attributes is an attribute group.

A matcher with no `when` applies to every log, which is often what you want:

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

Without the matcher, the attribute has nothing to compare with:

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

The attribute now resolves to its definition, so it gets the checks for that definition: type, stability, deprecation, and any annotation-based policy you have written.

`none -> signal: no match` is still on the line above. A log with no `event_name` names no signal, so this is expected rather than a gap, and live-check shows it in gray rather than yellow.

A log that names a declared event keeps that event, and the group is checked in addition.

## Metrics, which already match

A metric resolves its own signal by name. A matcher must not take that away, so leave `signal` out and add only the group:

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

`none -> signal:` means the metric's own name resolved the signal, not a matcher. The `when` on the name keeps the matcher away from metrics you did not write, such as `http.client.request.duration` from a dependency. Those are checked as before.

A metric's attributes are on its data points, so the group is checked against each point.

## Resources and instrumentation scopes

A resource is a set of attributes with no identifier and no signal that describes it. An attribute group is the only thing it can be compared with, so a resource matcher never has a `signal`.

Do not compare a resource with an entity. Entities are checked through the signals. When a metric declares `entity_associations`, live-check takes the attributes that entity needs from the resource and checks them as part of that metric. One message holds many signals that share one resource, so the resource carries the attributes of every entity those signals need. Compared with any single entity, the attributes the other signals need would all be reported as unexpected.

```toml
[[live-check.matchers]]
id = "match.resource"
sample_type = "resource"
when = '"service.name" in attributes && attributes["service.name"].startsWith("myapp.")'
attribute_groups = ["myapp.common"]
```

The `when` keeps the matcher away from resources that belong to other services.

An instrumentation scope is similar, with one difference: it has an identifier. Its name and version say which library produced the telemetry, so a scope matcher can use those directly.

```toml
[[live-check.matchers]]
id = "match.scope"
sample_type = "instrumentation_scope"
when = 'name.startsWith("myapp.")'
attribute_groups = ["myapp.common"]
```

The scope is often more useful inside other matchers than in one of its own. Any matcher can read `instrumentation_scope.name` and `instrumentation_scope.version`, so a signature can be limited to telemetry from your own instrumentation:

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

A span with the same attributes from another library no longer matches, and its `match_info` shows that nothing applied.

> **Note**: `instrumentation_scope` is only available for OTLP input today. On a JSON file or stdin it is null, so an expression that reads it without a guard errors on every sample.

## Where matchers go

Matchers describe the telemetry you emit, not the schema you define, so they belong in `.weaver.toml` and not in the registry. They are an array of tables, and they are evaluated in the order you write them.

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
| `when`             | No       | The matcher expression, in CEL. The matcher applies when it is true. Leave it out and the matcher applies to every sample of this type.               |
| `signal`           | No       | The one signal the sample is compared with. Leave it out to keep the natural match.                                                                    |
| `attribute_groups` | No | Attribute groups whose attributes are *permitted* on the sample, in priority order. Their definitions are used for the attribute checks, but an attribute missing from the sample is not reported. |
| `strict_attribute_groups` | No | Attribute groups whose requirement levels are *enforced*, so an attribute missing from the sample is reported. |

Matchers are a v2 feature. A matcher configured against a v1 registry stops the run at startup.

### What `signal` can name

`signal` is a plain id, and `sample_type` decides what kind of id it is.

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

Live-check looks the id up in your registry at startup. A name that is not there stops the run before any sample is read.

An attribute group never goes in `signal`. A group adds to the comparison rather than replacing it, so it always goes in one of the two group lists.

If any applied matcher names a group in `strict_attribute_groups`, the group is strict, even when an earlier matcher named it as permitted.

## The expression

`when` is written in [CEL](https://cel.dev), the Common Expression Language. CEL is made for this job: it is not Turing complete, and an expression cannot loop or reach outside the sample it is given. Every expression is compiled once at startup and then run against each sample.

An expression sees one sample and returns true or false. It can read these variables:

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
| `event_name`, `severity_text`, `severity_number`, `body` | Log                                             | The fields on the log record. All but `event_name` are optional. A field the record omits is null.                                                      |

CEL has the usual operators, `==`, `!=`, `&&`, `||`, `!` and brackets. It also has the string methods `matches`, `startsWith`, `endsWith` and `contains`, and the macros `has`, `exists`, `exists_one`, `all`, `map` and `filter`.

| Expression                               | What it does                                                                                                                                                                |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `"key" in attributes`                    | True if the attribute is on the sample.                                                                                                                                     |
| `attributes["key"] == "value"`           | True if the value is the one you give.                                                                                                                                      |
| `attributes["key"] in ["a", "b"]`        | True if the value is one of the ones you list.                                                                                                                              |
| `attributes["key"].startsWith("myapp.")` | Also `endsWith` and `contains`.                                                                                                                                             |
| `attributes["key"].matches("regex")`     | True if the value is a string and the regular expression matches it. The pattern is compiled on every sample, so use `in` or `startsWith` when they say the same thing.    |

### Guarding a read

In CEL, reading an attribute that is not on the sample is an error, not an empty value. So every value you read needs an `in` test on the same key:

```cel
"myapp.checkout.stage" in attributes
  && attributes["myapp.checkout.stage"] in ["cart", "payment", "confirm"]
```

When one side of an `&&` is false, CEL ignores an error on the other side. The order of the two sides does not matter.

The optional variables `severity_text`, `severity_number`, `body`, `resource` and `instrumentation_scope` are null on a sample that does not carry them. Reading a field of null is also an error, so guard them with `!= null`:

```cel
body != null && body.contains("declined")
```

`has(body)` does not compile, because the `has` macro takes a field selection such as `has(status.code)`, not a bare name.

Without the guard, the matcher errors on every sample that lacks the key and applies to none of them. The run still finishes. The coverage block reports the count and the first message:

```text
Matcher coverage
  - match.checkout: 0 sample(s), 1 error(s): No such key: myapp.checkout.stage
```

and two warnings land in the diagnostic report at the end of the run:

```text
  ⚠ Matcher `match.checkout` errored on 1 sample(s). First error: ...

  ⚠ Matcher `match.checkout` applied to no samples.
```

## Resolution order

For each sample:

1. The natural match is found first: a metric by its name, a log by its `event_name`.
2. Every matcher whose `sample_type` and `when` both pass is applied.
3. The first applied matcher with a `signal` sets it, replacing the natural match. A later matcher with a `signal` is ignored and named in the sample's `match_info`.
4. The attribute groups from every applied matcher are added in the order the matchers are written. Within one matcher, strict groups come before permitted ones. A group named twice is kept once.
5. Only the strict groups have their requirement levels enforced.
6. The sample and its attributes are compared with the signal and the groups together.

This is why a matcher that only adds attributes leaves `signal` out. If it named one, it would replace the signal that a metric or log already resolved by name.

## What a sample was checked against

Every sample's result holds a `match_info`: the signal, the matcher whose `signal` won, the attribute groups, and one entry per applied matcher. The ansi output puts one dimmed line under the sample for each thing a matcher contributed.

This output comes from a larger set of matchers than the examples above, to show several at once:

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

`none` means the sample's own name resolved the signal, or that nothing set one. `no match` is yellow on a sample that is expected to resolve a signal and has not: a span, a span event, a metric, or a log with an `event_name`. It is gray on a resource, a scope, a span link, a profile and a log with no `event_name`, because none of those name a signal. `(conflict, ignored)` is red.

`match_info` is not a finding, so it does not reach `finding_filters`, `fail_on` or the emitted OTLP logs.

## Related configuration

```toml
[live-check]
search_all_attributes = true
```

With this set, live-check also searches the base attribute definitions in your registry and its dependencies. An attribute found this way reports the `schema_url` that declares it, a hint that your registry can reference or import it.

Without it, a v2 registry compares an attribute with the signal and attribute groups of its match, and nothing else. The first example on this page shows the result: no match, so nothing to compare with. The bare attribute inputs, `--input-format text` and a JSON file of attributes alone, never have a match and need this setting. A v1 registry always searches every attribute it holds.

## Diagnostics

Every problem with a matcher is reported, either at startup or in the matcher coverage block at the end of a run.

| What happened                                                  | When you hear about it                      |
| -------------------------------------------------------------- | ------------------------------------------- |
| The expression does not parse                                  | Startup, run stops                          |
| `signal` or an attribute group name is not in the registry     | Startup, run stops                          |
| A matcher is configured against a v1 registry                  | Startup, run stops                          |
| The expression errors while running, e.g. an unguarded read    | Warning, with a count and the first message |
| The expression reads a variable that sample type does not have | Warning, with a count and the first message |
| A `matches` pattern is not a valid regex                       | Warning, with a count and the first message |
| A matcher applied to no samples                                | Warning                                     |

The coverage block lists how many samples each matcher applied to, so you can see which ones are useful:

```text
Matcher coverage
  - match.checkout: 1 sample(s)
  - match.legacy: 0 sample(s)
```

## New findings

| Finding                | Level       | When it is raised                                                                                                                                                                                                                                                                                      |
| ---------------------- | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `unexpected_attribute` | Improvement | A sample has an attribute that is not in the comparison set. A metric or a log has a comparison set as soon as its name resolves, so this is raised with no matchers configured. A span needs a matcher. When a matcher sets `signal`, the comparison set comes from that signal, not from the natural match. |
| `kind_mismatch`        | Violation   | A matched span has a different `kind` from the one on the span signal. This check is only possible once a span has a matcher.                                                                                                                                                                                 |
