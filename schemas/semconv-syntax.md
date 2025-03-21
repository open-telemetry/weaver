# Semantic Convention YAML Language

First, the syntax with a pseudo [EBNF](https://en.wikipedia.org/wiki/Extended_Backus-Naur_form) grammar is presented.
Then, the semantic of each field is described.

<!-- tocstart -->

<!-- toc -->

- [Semantic Convention YAML Language](#semantic-convention-yaml-language)
  - [JSON Schema](#json-schema)
  - [Syntax](#syntax)
  - [Semantics](#semantics)
    - [Groups](#groups)
    - [Semantic Convention](#semantic-convention)
      - [Span semantic convention](#span-semantic-convention)
      - [Event semantic convention](#event-semantic-convention)
      - [Event semantic convention example](#event-semantic-convention-example)
      - [Metric Group semantic convention](#metric-group-semantic-convention)
      - [Metric semantic convention](#metric-semantic-convention)
      - [Attribute group semantic convention](#attribute-group-semantic-convention)
      - [Any Value semantic convention](#any-value-semantic-convention)
    - [Attributes](#attributes)
      - [Examples (for examples)](#examples-for-examples)
      - [Ref](#ref)
      - [Type](#type)

<!-- tocstop -->

## JSON Schema

A JSON schema description of the syntax is available as [semconv.schema.json](./semconv.schema.json),
see [README.md](./README.md) for how to use it with an editor. The documentation
here in `syntax.md` should be considered more authoritative though. Please keep
`semconv.schema.json` in synch when changing the "grammar" in this file!

## Syntax

All attributes are lower case.

```ebnf
groups ::= semconv
       | semconv groups

semconv ::= id [convtype] brief [note] [extends] [stability] [deprecated] [display_name] [attributes]  [annotations] specificfields

extends_or_attributes ::= (extends | attributes | (extends attributes))

id    ::= string

convtype ::= "span" # Default if not specified
         |   "resource" # see spansfields
         |   "event"    # see eventfields
         |   "metric"   # see metricfields
         |   "attribute_group" # see attribute_group_fields

brief ::= string
note  ::= string

extends ::= string

stability ::= "stable"
          |   "development"
          |   "deprecated"
          |   "alpha"
          |   "beta"
          |   "release_candidate"

deprecated ::= renamed renamed_to [note]
           |   obsoleted [note]
           |   uncategorized [note]
          
renamed_to ::= string

display_name ::= string

annotations ::= string yaml
                
attributes ::= (id type brief examples | ref [brief] [examples]) [tag] stability [deprecated] [requirement_level] [sampling_relevant] [note] [annotations]

# ref MUST point to an existing attribute id
ref ::= id

type ::= simple_type
     |   template_type
     |   enum

simple_type ::= "string"
     |   "int"
     |   "double"
     |   "boolean"
     |   "string[]"
     |   "int[]"
     |   "double[]"
     |   "boolean[]"

template_type ::= "template[" simple_type "]" # As a single string

enum ::= members

members ::= member {member}

member ::= id value [brief] [note] stability [deprecated]

requirement_level ::= "required"
         |   "conditionally_required" <condition>
         |   "recommended" [condition] # Default if not specified
         |   "opt_in"

sampling_relevant ::= boolean

examples ::= <example_value> {<example_value>}

specificfields ::= spanfields
               |   eventfields
               |   metricfields
               |   attribute_group_fields

attribute_group_fields ::= extends_or_attributes

spanfields ::= [events] span_kind stability extends_or_attributes

eventfields ::= name [body] stability

body ::= any_value

any_value_type ::= "map"
         |   "string"
         |    "int"
         |    "double"
         |    "boolean"
         |    "string[]"
         |    "int[]"
         |    "double[]"
         |    "boolean[]"
         |    "byte[]"
         |    "enum"
         |    "undefined"

any_value ::= id any_value_type brief [examples] stability [deprecated] requirement_level [note] [fields] [members]

fields ::= any_value {any_value}

span_kind ::= "client"
          |   "server"
          |   "producer"
          |   "consumer"
          |   "internal"

events ::= id {id} # MUST point to an existing event group

name ::= string

metricfields ::= metric_name instrument unit stability

metric_name ::= string
instrument ::=  "counter"
            | "histogram"
            | "gauge"
            | "updowncounter"
unit ::= string
```

## Semantics

### Groups

Groups contain the list of semantic conventions and it is the root node of each yaml file.

### Semantic Convention

The field `semconv` represents a semantic convention and it is made by:

- `id`, string that uniquely identifies the semantic convention.
- `type`, optional enum, defaults to `span` (with a warning if not present).
- `brief`, string, a brief description of the semantic convention.
- `stability`, required enum, specifies the stability of the attribute.
- `note`, optional string, a more elaborate description of the semantic convention.
   It defaults to an empty string.
- `extends`, optional string, reference another semantic convention `id`.
   It inherits all attributes defined in the specified semantic convention.
- `deprecated`, optional, when present marks the semantic convention as deprecated.
   The string provided as `<description>` MUST specify why it's deprecated and/or what to use instead.
- `attributes`, list of attributes that belong to the semantic convention.
- `annotations`, optional map of annotations. Annotations are key-value pairs that provide additional information about
  the group. The keys are strings and the values are any YAML value.

#### Span semantic convention

The following is only valid if `type` is `span` (the default):

- `span_kind`, required enum, specifies the kind of the span.
- `events`, optional list of strings that specify the ids of
  event semantic conventions associated with this span semantic convention.

#### Event semantic convention

The following is only valid if `type` is `event`:

- `name`, required, string. The name of the event.
- `body`, optional, [`any value`](#any-value-semantic-convention). Describes the body of the event as an any_value type.

##### Event semantic convention example
  
  ```yaml
  - id: event.some_event
    name: the.event.name
    type: event
    brief: "Describes the event."
    stability: development
    attributes:                                  # Optional
      - ref: registry.attribute.id
      - ref: registry.some_other.attribute.id    # Reference to an existing global attribute
    body:                                        # Optional, follows the any_value conventions
      id: event_body.some_event.fields
      type: map
      requirement_level: required
      fields:                                    # Unique to this event definition only
        - id: method
          type: string
          stability: development
          brief: "The HTTP method used in the request."
          examples: ['GET', 'POST']
          requirement_level: required
        - id: url
          type: string
          stability: development
          brief: "The URL of the request."
          examples: ['http://example.com']
          requirement_level: required
        - id: status_code
          type: int
          stability: development
          brief: "The status code of the response."
          examples: [200, 404]
          requirement_level: required
        - id: nested_map
          type: map
          stability: development
          requirement_level: required
          fields:
            - id: nested_field
              type: string    # May be any supported any_value type
              stability: development
              requirement_level: required
              brief: "A nested field."
              examples: ['nested_value']
        - id: nested_enum_state
          type: enum
          stability: development
          requirement_level: required
          members:
            - id: active
              value: 'active'
              brief: The state became active.
            - id: inactive
              value: 'inactive'
              brief: The state became inactive.
            - id: background
              value: 'background'
              brief: The state is now in the background.
```

#### Metric Group semantic convention

Metric group inherits all from the base semantic convention, and does not
add any additional fields.

The metric group semantic convention is a group where related metric attributes
can be defined and then referenced from other `metric` groups using `ref`.

#### Metric semantic convention

The following is only valid if `type` is `metric`:

  - `metric_name`, required, the metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model).
  - `instrument`, required, the [instrument type]( https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/api.md#instrument)
  that should be used to record the metric. Note that the semantic conventions must be written
  using the names of the synchronous instrument types (`counter`, `gauge`, `updowncounter` and `histogram`).
  For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types).
  - `unit`, required, the unit in which the metric is measured, which should adhere to
    [the guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units).

#### Attribute group semantic convention

Attribute group (`attribute_group` type) defines a set of attributes that can be
declared once and referenced by semantic conventions for different signals, for example spans and logs.
Attribute groups don't have any specific fields and follow the general `semconv` semantics. `stability` is not required for attribute groups.

#### Any Value semantic convention

Describes the type of the value of an extended (log) attribute or the body of an event.

- `id`, required, string. The name of the field / any value.
- `type`, either a string literal denoting the type as a primitive or an array type, [an enum definition](#enumeration) or a map of fields.  Required.
   The accepted string literals are:
  * `"string"`: String value.
  * `"int"`: Integer value.
  * `"double"`: Double value.
  * `"boolean"`: Boolean value.
  * `"string[]"`: Array of strings value.
  * `"int[]"`: Array of integer value.
  * `"double[]"`: Array of double value.
  * `"boolean[]"`: Array of boolean value.
  * `"byte[]"`: Array of bytes value.
  * `"map"`: Map of any_value types.
    * The `fields` field is required and contains a list of any_value entries that describe each field of the map.
  * `"enum"`: Enumerated value.
    * The `members` field is required and contains a list of enum entries.
  * `"undefined"`: The actually format of the value is not defined.
- `brief`, `note`, `deprecated`, `stability`, same meaning as for the whole
  [semantic convention](#semantic-convention), but per field.
- `requirement_level`, required. Specifies if the field is mandatory.
   Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended".
    When set to "conditionally_required", the string provided as `<condition>` MUST specify
    the conditions under which the field is required.
- `examples`, sequence of example values for the field or single example value.
   They are required only for string and string array fields.
   Example values must be of the same type of the field or for a map of fields, the type can be of a string type.
   If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. See [below](#examples-for-examples).
- `fields`, required only when the type is `map`, list of any value entries that describe each field of the map.
- `members`, required only when the type is `enum`, list of enum entries. See [below](#enumeration).

### Attributes

An attribute is defined by:

- `id`, string that uniquely identifies the attribute. Required.
- `type`, either a string literal denoting the type as a primitive or an array type, a template type or an enum definition (See later).  Required.
   The accepted string literals are:
  * _primitive and array types as string literals:_
    * `"string"`: String attributes.
    * `"int"`: Integer attributes.
    * `"double"`: Double attributes.
    * `"boolean"`: Boolean attributes.
    * `"string[]"`: Array of strings attributes.
    * `"int[]"`: Array of integer attributes.
    * `"double[]"`: Array of double attributes.
    * `"boolean[]"`: Array of boolean attributes.
  * _template type as string literal:_ `"template[<PRIMITIVE_OR_ARRAY_TYPE>]"` (See [below](#template-type))
  See the [specification of Attributes](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/common/README.md#attribute) for the definition of the value types.
- `stability`, required enum, specifies the stability of the attribute.
- `ref`, optional string, reference an existing attribute, see [below](#ref).
- `tag`, optional string, associates a tag ("sub-group") to the attribute.
   It carries no particular semantic meaning but can be used e.g. for filtering
   in the markdown generator.
- `requirement_level`, optional, specifies if the attribute is mandatory.
   Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the attribute is "recommended".
   When set to "conditionally_required", the string provided as `<condition>` MUST specify
   the conditions under which the attribute is required.
- `sampling_relevant`, optional boolean,
  specifies if the attribute is (especially) relevant for sampling and
  thus should be set at span start. It defaults to `false`.
- `brief`, `note`, `deprecated`, same meaning as for the whole
  [semantic convention](#semantic-convention), but per attribute.
- `examples`, sequence of example values for the attribute or single example value.
   They are required only for string and string array attributes.
   Example values must be of the same type of the attribute.
   If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. See [below](#examples-for-examples).
- `annotations`, optional map of annotations. Annotations are key-value pairs that provide additional information about
  the attribute. The keys are strings and the values are any YAML value.

#### Examples (for examples)

Examples for setting the `examples` field:

A single example value for a string attribute. All the following three representations are equivalent:

```yaml
examples: 'this is a single string'
```

or

```yaml
examples: ['this is a single string']
```

or

```yaml
examples:
   - 'this is a single string'
```

Attention, the following will throw a type mismatch error because a string type as example value is expected and not an array of string:

```yaml
examples:
   - ['this is an error']

examples: [['this is an error']]
```

Multiple example values for a string attribute:

```yaml
examples: ['this is a single string', 'this is another one']
```

or

```yaml
examples:
   - 'this is a single string'
   - 'this is another one'
```

A single example value for an array of strings attribute:

```yaml
examples: [ ['first element of first array', 'second element of first array'] ]
```

or

```yaml
examples:
  - ['first element of first array', 'second element of first array']
```

Multiple example values for an array of string attribute:

```yaml
examples: [ ['first element of first array', 'second element of first array'], ['first element of second array', 'second element of second array'] ]
```

or

```yaml
examples:
   - ['first element of first array', 'second element of first array']
   - ['first element of second array', 'second element of second array']
```

Attention: the following will throw a type mismatch error because an array of strings as type for the example values is expected and not a string:

```yaml
examples: 'this is an error'
```

#### Ref

`ref` MUST have an id of an existing attribute. When it is set, `id`, `type`, `stability`, and `deprecation` MUST NOT be present.
`ref` is useful for specifying that an existing attribute of another semantic convention is part of
the current semantic convention and inherit its `brief`, `note`, and `example` values. However, if these
fields are present in the current attribute definition, they override the inherited values.

#### Type

An attribute type can either be a string, int, double, boolean, array of strings, array of int, array of double,
array of boolean, a template type or an enumeration.

##### Template type

A template type attribute represents a _dictionary_ of attributes with a common attribute name prefix. The syntax for defining template type attributes is the following:

`type: template[<PRIMITIVE_OR_ARRAY_TYPE>]`

The `<PRIMITIVE_OR_ARRAY_TYPE>` is one of the above-mentioned primitive or array types (_not_ an enum) and specifies the type of the `value` in the dictionary.

The following is an example for defining a template type attribute and it's resolution:

```yaml
groups:
  - id: trace.http.common
    type: attribute_group
    brief: "..."
    attributes:
      - id: http.request.header
        type: template[string[]]
        stability: stable
        brief: >
          HTTP request headers, the key being the normalized HTTP header name (lowercase, with `-` characters replaced by `_`), the value being the header values.
        examples: ['http.request.header.content_type=["application/json"]', 'http.request.header.x_forwarded_for=["1.2.3.4", "1.2.3.5"]']
        note: |
          ...
```

In this example the definition will be resolved into a dictionary of attributes `http.request.header.<key>` where `<key>` will be replaced by the actual HTTP header name, and the value of the attributes is of type `string[]` that carries the HTTP header value.

##### Enumeration

If the type is an enumeration, additional fields are required:

- `members`, list of enum entries.

An enum entry has the following fields:

- `id`, string that uniquely identifies the enum entry.
- `value`, string, int, or boolean; value of the enum entry.
- `brief`, optional string, brief description of the enum entry value. It defaults to the value of `id`.
- `note`, optional string, longer description. It defaults to an empty string.
- `stability`, required stability level. Attributes marked non-stable cannot have stable members.
- `deprecated`, optional string, similarly to semantic convention and attribute deprecation, marks specific member as deprecated.

