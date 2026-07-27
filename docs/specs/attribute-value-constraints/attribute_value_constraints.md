# Constraining Attribute Values

Status: Work in Progress

An attribute reference can update an attribute's description, requirement level, and examples for one signal. It
cannot say **which value the attribute takes there**, even though the signal often fixes that value: a Cassandra
query always sets `db.system.name` to `cassandra`, an OpenAI call always sets `gen_ai.provider.name` to `openai`.

With no way to model it, convention authors write it in prose:

```yaml
- ref: db.system.name
  note: MUST be set to `"cassandra"`.
  examples: ['cassandra']
```

That sentence, or a close variant, appears **around 110 times** in the OpenTelemetry semantic conventions;
`messaging.system` alone carries 37 near-identical copies.

The same gap shows up when the value is not fixed but the set of values is known. `error.type` is defined with a
single `_OTHER` member and asks every user of it to fill in the rest, in prose:

> Instrumentations SHOULD document the list of errors they report.

The only way to comply is more prose. The .NET socket connect span lists its 16 error codes in a `note`:

```yaml
- ref: error.type
  brief: "Socket error code."
  note: |
    The following errors codes are reported:

    - `network_down`
    - `address_already_in_use`
    # ... 14 more
  examples: ["connection_refused", "address_not_available"]
```

Prose is invisible to Weaver: nothing validates it, code generation cannot turn it into a constant or an enum,
the markdown templates cannot render it in the attribute table, and `live-check` cannot check real telemetry
against it.

This document proposes to model value constraints instead. It covers three problems. Each one is useful on its
own and can ship on its own, so each gets its own phase.

| # | Problem | Field | Phase | Reference sites today |
|---|---|---|---|---|
| 1 | This attribute always has one specific value in this context | `constant` | 1 | ~110 |
| 2 | Which refinement does an observed piece of telemetry belong to? | `discriminator` | 2, needs phase 1 | ~63 refinements |
| 3 | This reference draws from a limited set, though the global definition is open-ended | `type.members` | 3 | ~32 |

## Problem 1: a single, fixed value

Tracked by [weaver#1617](https://github.com/open-telemetry/weaver/issues/1617) and
[weaver#803](https://github.com/open-telemetry/weaver/issues/803).

This is by far the most common case, and the attribute is almost always a *system identifier* — one that says
which technology the signal describes.

| Attribute | Kind | Sites | Scenario |
|---|---|---|---|
| `messaging.system` | enum | 37 | one refinement per messaging system: Kafka, Service Bus, SQS, Pub/Sub, RabbitMQ, RocketMQ |
| `hw.type` | enum | 21 | one metric refinement per hardware component: cpu, gpu, battery, fan, … |
| `db.system.name` | enum | ~17 | one span per database: PostgreSQL, MySQL, Redis, DynamoDB, … |
| `messaging.operation.name` | **string** | 13 | the operation a messaging span describes |
| `rpc.system.name` | enum | 10 | one span pair (client + server) per RPC system: gRPC, Connect, JSON-RPC, Dubbo |
| `messaging.operation.type` | enum | 6 | one span per operation category: create, send, receive, process, settle |
| `gen_ai.provider.name` | enum | 5 | one refinement per GenAI provider: OpenAI, Anthropic, Azure AI Inference, AWS Bedrock |
| `faas.trigger` | enum | 2 | one span per serverless trigger kind: datasource, timer |
| `azure.resource_provider.namespace` | **string** | 2 | the Azure service behind the call |

Two of these break assumptions an obvious design would make.

**`messaging.operation.name` and `azure.resource_provider.namespace` are plain strings, not enums.** A design
that works by picking an enum member cannot express them at all, which rules out the `type`-refinement shapes
discussed in [weaver#479](https://github.com/open-telemetry/weaver/issues/479). The constraint belongs on the
*reference*, next to `brief` and `requirement_level`, not on the attribute's type.

**`faas.trigger` is pinned on ordinary spans, not on refinements.** `faas.datasource.server` and
`faas.timer.server` are separate span types rather than refinements of `faas.server`, because serverless
platforms specialize each of them further. They still need to say `faas.trigger = datasource`, so this cannot be
a refinement-only feature.

### Proposal: `constant` (phase 1)

```yaml
span_refinements:
  - id: aws.sqs.producer.send
    ref: messaging.producer.send
    brief: Describes a producer sending one or more messages to Amazon SQS.
    attributes:
      - ref: messaging.system
        constant: aws.sqs
```

This replaces the `note` + `examples` pair the same reference carries today; Weaver derives the examples from the
constant.

The same syntax works outside refinements — here the spans are siblings rather than a family:

```yaml
spans:
  - type: faas.datasource.server
    kind: server
    attributes:
      - ref_group: faas.attributes.server
      - ref: faas.trigger
        constant: datasource
```

## Problem 2: identifying a refinement

A span's identity is its type. A refinement, however, **has no identity in the telemetry it describes**: its name
appears nowhere on the wire. A Cassandra query and a generic database query are both just `db.query.client`. The
only thing that distinguishes them is an attribute value — `db.system.name = cassandra`.

So no rule a refinement adds can be enforced today. Given a real span, `live-check` cannot tell which
refinement's rules to apply, so it only checks the general signal. A backend reading the resolved registry cannot
route the span to the more specific schema. Code generation cannot emit a per-system helper.

Roughly **63 refinements** exist today that differ from their general signal only by a pinned attribute — the
messaging, hardware, and GenAI families — and another ~27 database and RPC spans will join them as they move to
the new format. `gen_ai.provider.name` is the clearest example: the provider name *is* the reason the OpenAI,
Anthropic, and Bedrock refinements exist, and it is required on all of their spans.

Not every pinned value plays this role, so it has to be stated rather than guessed. The OpenAI refinement also
pins `azure.resource_provider.namespace`, but that attribute is optional and identifies nothing — matching on it
would fail on telemetry that is perfectly valid.

### Proposal: `discriminator` (phase 2)

A signal declares which of its attributes identify its refinements. Each refinement pins the values of those
attributes with `constant`, so this phase builds on phase 1.

```yaml
spans:
  - type: gen_ai.inference.client
    kind: client
    discriminator: [gen_ai.provider.name]
    # ...

span_refinements:
  - id: openai.inference.client
    ref: gen_ai.inference.client
    attributes:
      - ref: gen_ai.provider.name
        constant: openai
```

Weaver can then validate that:

- discriminating attributes are required on both the general signal and its refinements;
- the combination of their values is unique among all refinements visible in this registry.

## Problem 3: a known limited set

Tracked by [weaver#479](https://github.com/open-telemetry/weaver/issues/479), which eleven TODOs across the
semantic conventions point at — `db.system.name` and `messaging.system` are kept out of common attribute groups
because of it, and notes that want to be YAML stay prose.

Here the value is not fixed, but the set of values used in this context is much smaller than the one the global
definition allows.

| Attribute | Kind | Sites | Distinct sets | Scenario |
|---|---|---|---|---|
| `error.type` | open string enum | ~6 | 6 | the failure modes a specific operation can actually report |
| `messaging.operation.name` | **string** | 4 | 4 | the operation names a specific broker uses |
| `gen_ai.operation.name` | enum, 16 members | 2 | 2 | which GenAI operations a given span type covers |
| `hw.state` | enum, 5 members | 14 | 5 | health states differ per component: a battery can be `charging`, an enclosure can be `open` |
| `cpu.mode` | enum, 8 members | 6 | 3 | a container reports 2 modes, a process 3, a host 7 |

**The most important case is `error.type`.** Its definition has a single `_OTHER` member and asks every user of
it to document the errors it reports, so *every* real set lives at a reference site. It also goes well beyond the
~6 sites counted above: any operation that can fail should list its failure modes, and today the only way to do
that is the prose list shown in the introduction.

**`messaging.operation.name` is the interesting one.** It is a *string* on purpose: globally, the set of
messaging operation names is open, because it is whatever the broker calls its own operations. At the reference
site the set is known — an Azure Service Bus "create" span uses `send` or `schedule`, and nothing else. That is
the shape the design has to support: **open at definition time, known at reference time.**

Reference sites also need to add values the global definition does not have. `messaging.operation.name` lives in
the core registry, while the refinements using it can live in another one and should not require changes to the
registry they do not own.

### Proposal: `type.members` on a reference (phase 3)

A reference may restate the attribute's members. Two kinds of entry cover every case in the table above:

- `- ref: <member>` selects a member the definition already has. It inherits the member's properties and may
  update the ones a reference is allowed to update.
- `- id: <value>` adds a member the definition does not have, and must carry `brief` and `stability` like any
  other public definition.

A reference may do this even when the definition is a plain string — it refines the string into a locally
documented enum. That is safe because a string enum carries string values: the wire type never changes, only the
documentation and what code generation can emit. It is also the honest model for `messaging.operation.name`,
which is a string globally because no global vocabulary exists, and a small known set everywhere it is used:

```yaml
attributes:
  - key: messaging.operation.name
    type: string
    stability: development
    brief: The system-specific name of the messaging operation.

span_refinements:
  - id: azure.servicebus.producer.create
    ref: messaging.producer.create
    attributes:
      - ref: messaging.operation.name
        brief: Azure Service Bus operation name.
        type:
          members:
            - id: send
              brief: Sends a message to a queue or topic.
              stability: development
            - id: schedule
              brief: Schedules a message for future delivery.
              stability: development
```

When the values already exist in the definition, the entries carry nothing but refs — here the GenAI inference
span narrows the definition to a subset of its members:

```yaml
attributes:
  - key: gen_ai.operation.name
    stability: development
    brief: The name of the operation being performed.
    type:
      members:
        - id: chat
          value: chat
          brief: Chat completion operation such as OpenAI Chat API.
          stability: development
        - id: embeddings
          value: embeddings
          brief: Embeddings operation such as OpenAI Create embeddings API.
          stability: development
        # ... 14 more

spans:
  - type: gen_ai.inference.client
    kind: client
    attributes:
      - ref: gen_ai.operation.name
        type:
          members:
            - ref: chat
            - ref: generate_content
            - ref: text_completion
```

`error.type` needs the same field for the opposite reason — none of its values exist in the definition. Here is
the .NET socket span from the introduction, with each code described instead of listed in prose:

```yaml
attributes:
  - key: error.type
    stability: stable
    brief: Describes a class of error the operation ended with.
    type:
      members:
        - id: other
          value: _OTHER
          brief: A fallback error value used when the instrumentation has no custom value.
          stability: stable

spans:
  - type: dotnet.socket.connect.internal
    kind: internal
    attributes:
      - ref: error.type
        brief: Socket error code.
        type:
          members:
            - id: network_down
              brief: The network subsystem is unavailable.
              stability: development
            - id: connection_refused
              brief: The remote host actively refused the connection.
              stability: development
            # ... 14 more
```

There is no "exclude these members" form: `type.members` on a reference replaces the members of the definition.
A new member added to `gen_ai.operation.name` must **not** silently become part of a refinement's set — someone
has to decide which span types it belongs to.

## Compatibility

- Adding a `constant` to an existing signal is **not** a breaking change; it records what the prose already said.
  Weaver cannot check that the value matches the prose it replaces — that is a review task during the migration.
- Changing an existing `constant` value **is** a breaking change.
- Changing which attributes discriminate a signal **is** a breaking change.
- Narrowing or extending a known set is **not** a breaking change. The set is open, so no telemetry stops
  conforming; only documentation and validator advice change.

## Out of Scope

- **Turning enums into standalone named types** shared by several attributes. Unrelated to this proposal; the
  fields above work either way.

## Appendix: Related Work

- [weaver#1617](https://github.com/open-telemetry/weaver/issues/1617) — Indicate which attributes will have a
  constant value. The original request behind Problem 1.
- [weaver#803](https://github.com/open-telemetry/weaver/issues/803) — Allow specifying fixed values for
  attributes in refinements. The same ask, scoped to refinements.
- [weaver#479](https://github.com/open-telemetry/weaver/issues/479) — Allow updating enum values when referencing
  an attribute. A long design discussion that phase 3 is intended to close.
- [weaver#520](https://github.com/open-telemetry/weaver/issues/520) — Removed the flag that made enums closed.
  This document assumes enums are open because of it.
- [weaver#1590](https://github.com/open-telemetry/weaver/issues/1590) — Don't allow refining refinements. Phase 2
  relies on that.
- [weaver#892](https://github.com/open-telemetry/weaver/issues/892) — Re-design type definitions. Overlaps
  phase 3 and the named-enum item under Out of Scope.
- [weaver#878](https://github.com/open-telemetry/weaver/issues/878) — Deprecating enum members, and
  [weaver#1146](https://github.com/open-telemetry/weaver/issues/1146) — stability required on members. Both apply
  to members added at a reference site in phase 3.
- [weaver#1623](https://github.com/open-telemetry/weaver/issues/1623) — Weaver filters return refinements. The
  template side of Problem 2.
- [weaver#329](https://github.com/open-telemetry/weaver/issues/329) — Arrays of enum values.
- [semconv#3904](https://github.com/open-telemetry/semantic-conventions/pull/3904) — The messaging refactor that
  produced 37 copies of the same note.
