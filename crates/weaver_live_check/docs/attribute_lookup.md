# Attribute lookup

Before live-check can judge an attribute it has to find the definition that
applies to it. That definition decides the advice: stability, deprecation, value
type, allowed enum members, and whatever a custom policy makes of its
annotations. The choice is not obvious, because the same key can be defined in
more than one place and a signal may adjust a definition for its own use. This
document covers the search order, how it differs between the two registry
formats, and how the report statistics follow from it.

## Two example registries

`platform` holds shared definitions. `checkout` is a service registry that
defines nothing and imports what it needs. Both are cut down to what the
examples need.

```yaml
# platform: the shared registry
file_format: definition/2

attributes:
  - { key: service.name, type: string, stability: stable, brief: "Service name." }
  - { key: deployment.environment, type: string, stability: stable, brief: "Environment." }
  - { key: server.address, type: string, stability: stable, brief: "Server address." }
  - { key: db.query.text, type: string, stability: stable, brief: "The query text." }
  - { key: http.request.header, type: "template[string]", stability: stable, brief: "Request headers." }
  - { key: messaging.destination.name, type: string, stability: stable, brief: "Destination." }

attribute_groups:
  - id: deployment
    visibility: public
    brief: Where the service runs.
    stability: stable
    attributes:
      - ref: deployment.environment

entities:
  - type: service
    brief: A service instance.
    stability: stable
    requirement_level: recommended
    identity:
      - ref: service.name

metrics:
  - name: http.server.request.duration
    brief: Duration of HTTP server requests.
    instrument: histogram
    unit: s
    stability: stable
    requirement_level: recommended
    attributes:
      - ref: server.address
      - ref: http.request.header
        annotations: { redact: true }   # applies to this metric only

spans:
  - type: db.query
    kind: client
    brief: A database query.
    stability: stable
    requirement_level: recommended
    name: { note: "{db.query.text}" }
    attributes:
      - ref: server.address
      - ref: db.query.text

  - type: messaging.publish
    kind: producer
    brief: Publishing a message.
    stability: stable
    requirement_level: recommended
    name: { note: "{messaging.destination.name}" }
    attributes:
      - ref: messaging.destination.name
```

```yaml
# checkout: defines nothing, imports from platform
file_format: definition/2

imports:
  metrics:          [http.server.request.duration]
  spans:            [db.query]
  entities:         [service]
  attribute_groups: [deployment]
```

Note that `checkout` imports `db.query` but not `messaging.publish`, and that
the metric annotates `http.request.header` while the registry-level definition
of that template does not. Annotations reach the advisors as part of the
definition and a Rego policy can read them as
`input.registry_attribute.annotations`, so the lookup decides whether such a
policy fires.

## Registry structure

A v2 registry holds its own `attributes`, its public `attribute_groups`, its
signals, and a `dependencies` chain of the registries it imports from. Signals
hold no definitions: each signal attribute references a definition in some
registry's `attributes`, local or further down the chain. A signal may override
parts of that definition for itself (annotations, brief, note, examples,
requirement level, deprecation, stability), where annotations merge and
everything else replaces. This is a *refinement*, and it belongs to that one
signal.

A v1 registry is flat. Resolution merges the registry and its imports into one
list of groups, with no separate attribute list and no record of origin.

## The search order

A sample has a *matched signal* when its signal name matches one in the
registry. Attributes on a resource, on an instrumentation scope, or on an
unrecognised metric or event have none.

> **Spans and profiles are never matched signals.** A span sample carries only
> its runtime name, such as `SELECT orders`, which cannot be mapped back to the
> `db.query` span type. A profile carries no name at all. Steps 1 and 2 are
> never reached for a span, a span event, a span link or a profile, so these
> samples start at step 3. See [Spans](#spans) and [Profiles](#profiles).

With a matched signal, the search tries six steps in order and stops at the
first hit:

| Step | Source | Match |
|---|---|---|
| 1 | Matched signal | Exact key |
| 2 | Matched signal | Template, longest prefix |
| 3 | Registry under check | Exact key |
| 4 | Registry under check | Template, longest prefix |
| 5 | Dependency closure | Exact key |
| 6 | Dependency closure | Template, longest prefix |

Three rules produce that order. A matched signal is the strongest available
statement about an attribute, so it comes first. The registry under check is
searched before anything it merely depends on. And within each pair an exact key
is more specific than a prefix, so it beats a template.

Without a matched signal the search starts at step 3.

The pairs are strictly ordered, which is easy to get wrong in both directions:

- A template in the registry under check beats an exact key that only a
  dependency defines. Exact beats template only within a pair, and step 4 runs
  before step 5.
- A shorter template in the registry under check beats a longer one in a
  dependency. Length only decides between templates found in the same step.

Within the dependency closure, nearer wins: the closure is walked
nearest-first, and the first definition of a key is the one kept.

Because steps 1 and 2 come first, a refinement only reaches samples on the
signal that made it. `http.request.header.accept` on
`http.server.request.duration` matches that metric's template at step 2 and
arrives annotated `redact: true`. The same key on a `db.query` span reaches
step 6, matches the registry-level template, and carries no annotation.

### What steps 3 to 6 search

Four maps: an exact map and a template map for the registry under check, and the
same pair for its dependency closure. Each is built from `attributes` plus the
attributes of public attribute groups. The closure maps are filled
nearest-first, and the first definition of a key is kept, so a nearer dependency
shadows a further one.

Signal attributes are in none of them. An attribute on a signal is scoped to
that signal, and putting it in a registry-wide map would leak its refinement
onto unrelated samples. Nothing is lost, because a signal references its
attributes rather than defining them, so the definition is already in one of the
four.

Steps 5 and 6 mean an attribute a dependency defines resolves even when the
importing registry never mentions it. Sending `messaging.destination.name` to
`checkout` draws no `missing_attribute`. Whether it counts towards coverage is a
separate question, covered below.

### Attributes the signal never declared

Telemetry often carries more than the registry asked for. Steps 1 and 2 miss and
the search falls through to the registry, where the attribute gets the usual
advice if anything in the chain defines it. There is no `missing_attribute` and
no complaint about being unexpected, because a signal's attribute list is not
treated as exhaustive. `deployment.environment` on
`http.server.request.duration` works this way, found in the `deployment` group.
It gets no refinement, since the refinement was about a different attribute.

Step 2 still applies to extras: an undeclared attribute matching a template the
signal declares is signal-scoped, and carries that template's refinement.

The reverse case, an attribute the signal declares but the sample omits, is a
separate check reporting `required_attribute_not_present` or
`recommended_attribute_not_present` against the data point.

## Attribute groups

An **internal** group is a composition helper, letting several signals share a
block of references. Resolution copies its attributes into each signal that
references it and discards the group, so it never appears in a resolved
registry. Its attributes reach a sample only through those signals.

A **public** group is part of what a registry publishes. It needs a `brief` and
a `stability`, other registries can import it by name, and it survives
resolution. Live-check treats every public group as a registry-wide source of
definitions and counts its attributes in the statistics. An importing registry
chooses its groups through `imports`.

This makes a public group a good way to declare what a registry expects to see,
especially one that defines nothing itself. Instrumentation-scope attributes are
the clearest case, since no signal declares them and no entity carries them.
(Resource attributes are usually covered by their entity; `service.name` reaches
`checkout` through the imported `service` entity.)

```yaml
attribute_groups:
  - id: checkout.expected
    visibility: public
    brief: Attributes this service expects to emit.
    stability: stable
    attributes:
      - ref: messaging.destination.name
```

A `ref` resolves against any attribute in the dependency chain, so this pulls in
definitions the registry does not own.

## Spans

Matching depends on naming the signal a sample came from. Metrics and events
carry their registry name; spans carry a runtime name, and there is no way back
from `SELECT orders` to the `db.query` span type.

Live-check does what the data allows. It cannot match a sample to an individual
span, but every span sample is a span, and the registry's span definitions
together describe what spans may carry. Live-check pools them: **the attributes
of all span definitions behave exactly like the attributes of a public attribute
group.** They are registry-wide definitions, available from step 3 onwards to
any sample, and they count towards the registry surface.

Pooling costs precision. An attribute only `db.query` declares is not flagged
when it appears on an HTTP metric, because pooling discards the association.
Recovering it means tracking each attribute's originating signal and checking
the sample's carrier, which is a lot of machinery for a check that cannot be
done properly while spans stay unidentifiable. A stable key for spans is
[proposed for the specification](https://github.com/open-telemetry/opentelemetry-specification/pull/5233);
if it lands, spans can be matched like any other signal and this section becomes
unnecessary.

### Refinements on span attributes

A span may refine what it declares, so pooling has to choose a definition.
Live-check uses the **original definition**, the one in some registry's
`attributes`, and ignores the span's refinement. A refinement is a claim about
one signal's use of an attribute, sound only when the sample is known to belong
to that signal. A span sample never is, so applying it would attach one span's
opinion to samples from anywhere.

No special case implements this. The registry-wide map is built from
`attributes` and public groups, never from signals, so it holds the original by
construction, and a span attribute is always present because references resolve
somewhere in the chain.

## Profiles

A profile sample carries the attributes of the OTLP profiles dictionary and
little else live-check can use. A registry has no profile signal to name, so
there is nothing to match against and nothing to pool.

Profile attributes are searched from step 3 onwards, like any attribute with no
matched signal, and they land in the statistics on the same rules as the rest. A
profile attribute counts towards coverage when the registry declares it
somewhere else, falls to `seen_dependency_attributes` when only a dependency
defines it, and draws `missing_attribute` when nothing in the chain does. To put
the attributes a profile carries on the registry surface, reference them from a
public attribute group.

## v1 registries

The six-step order applies to v1 as well. A matched metric or event is itself a
group holding its own attributes, so steps 1 and 2 work. Steps 3 and 4 differ:
they search a map built from the attributes of *every* group, since a flat group
list is all a resolved v1 registry has. Three consequences follow.

Signal attributes are in the registry-wide map, so a refinement can reach other
carriers. Checked against v1, `http.request.header.accept` comes back annotated
`redact: true` on the `db.query` span as well as on the metric.

Duplicate keys tie-break on list order rather than anything meaningful, and not
even consistently: the last group to define an exact key wins, while for a
template it is the first.

There is no dependency chain to search. Resolution keeps only imported
definitions that something references, so an unreferenced one is absent and
live-check reports it missing.

## Statistics

The report sorts seen attributes into three records, using one idea: the
**registry surface**, the set of attributes a registry is responsible for. It
holds the registry's `attributes`, its public attribute group attributes, and
the attributes its signals and entities declare, spans included on the pooling
above. Deprecated attributes are excluded. Attributes on imported signals count:
`checkout` imports `http.server.request.duration`, so it has taken on that
metric and everything it declares.

The surface is wider than what the lookup searches, and the two answer
different questions. The surface asks "is this registry accountable for this
attribute?", about ownership; the lookup asks "which definition applies to this
sample?", about scope. So `db.query.text` is on `checkout`'s surface via the
imported `db.query` span, while a sample of it resolves at step 5 against
`platform`'s plain definition.

The three records:

- **`seen_registry_attributes`**: part of the registry surface.
- **`seen_dependency_attributes`**: outside the surface, but defined somewhere in
  the dependency chain.
- **`seen_non_registry_attributes`**: defined nowhere. Unknown, and it drew a
  `missing_attribute` finding.

Surface membership is checked before the definition's origin. Otherwise a
registry that imports all of its signals would have a full denominator and an
empty numerator, and could never report coverage at all.

`registry_coverage` is the fraction of the surface the telemetry reached,
counting the registry's metrics and events alongside its attributes. Only
`seen_registry_attributes` contributes, so a dependency attribute neither helps
nor hurts the score. It answers a different question: real and correctly used,
but is it something this registry claims?

## A worked example

This telemetry against `checkout`, with a policy raising `must_redact` when the
resolved definition is annotated `redact: true`:

| Carrier | Attribute | Findings |
|---|---|---|
| `http.server.request.duration` | `server.address` | none |
| `http.server.request.duration` | `http.request.header.accept` | `must_redact`, `template_attribute` |
| `http.server.request.duration` | `deployment.environment` | none |
| span `SELECT orders` | `db.query.text` | none |
| span `SELECT orders` | `http.request.header.accept` | `template_attribute` |
| no carrier | `messaging.destination.name` | none |
| no carrier | `checkout.cart.id` | `missing_attribute` |

The two `http.request.header.accept` rows give the search order in one pair. On
the metric it resolves at step 2 against the metric's own template and arrives
annotated, so the policy fires; on the span it falls to step 6, picks up the
plain registry template, and does not. `deployment.environment` shows the
fall-through: the metric never declares it, so step 3 finds it in the
`deployment` group of `checkout`.

```
seen_registry_attributes     4 of 5 seen, service.name was never emitted
seen_registry_metrics        http.server.request.duration: 1
seen_dependency_attributes   messaging.destination.name: 1
seen_non_registry_attributes checkout.cart.id: 1
registry_coverage            0.8333
```

`checkout`'s surface is five attributes: `server.address` and
`http.request.header` from the imported metric, `db.query.text` from the
imported span, `service.name` from the imported entity, and
`deployment.environment` from the imported group. Four were seen, plus the one
imported metric, giving five of six.

`service.name` is the informative miss: `checkout` claims it and never emitted
it, which is what coverage is for. `messaging.destination.name` is the opposite,
real and correctly used but reaching `checkout` only through a span it did not
import. `checkout.cart.id` is defined nowhere.

Against a v1 resolution of `checkout`, coverage is unchanged at 0.8333 but two
rows move. The span's `http.request.header.accept` now raises `must_redact`,
having picked up the metric's annotation through the flat map. And
`messaging.destination.name` becomes a `missing_attribute`, because nothing
`checkout` imports references it, so its definition did not survive resolution.
