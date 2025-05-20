# Template DataModels

## AnyValueSpec

The AnyValueTypeSpec is a specification of a value that can be of any type.

It will have one of the following values: 

**An Object:** A boolean attribute.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** A integer attribute (signed 64 bit integer).

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** A double attribute (double precision floating point (IEEE 754-1985)).

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** A string attribute.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** An array of strings attribute.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** An array of integer attribute.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** An array of double attribute.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** An array of boolean attribute.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** The value type is a map of key, value pairs

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| fields | [`AnyValueSpec`](#AnyValueSpec)[] | The collection of key, values where the value is an `AnyValueSpec` |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** The value type is a map of key, value pairs

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| fields | [`AnyValueSpec`](#AnyValueSpec)[] | The collection of key, values where the value is an `AnyValueSpec` |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** The value type will just be a bytes.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** The value type is not specified.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |

**An Object:** An enum definition type.

| field | type | description |
| --- | --- | --- |
| brief | `String` | A brief description of the value |
| examples | [`Examples`](#Examples) or `null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| id | `String` | String that uniquely identifies the enum entry. |
| members | [`EnumEntriesSpec`](#EnumEntriesSpec)[] | List of enum entries. |
| note | `String` | A more elaborate description of the value. It defaults to an empty string. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the value. |
| type | `String` | |


## Attribute

An attribute definition.

| field | type | description |
| --- | --- | --- |
| annotations | Object or Null | Annotations for the group. |
| brief | `String` | A brief description of the attribute. |
| deprecated | [`Deprecated`](#Deprecated) or `null` | Specifies if the attribute is deprecated. |
| examples | [`Examples`](#Examples) or `null` | Sequence of example values for the attribute or single example value. They are required only for string and string array attributes. Example values must be of the same type of the attribute. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| name | `String` | Attribute name. |
| note | `String` | A more elaborate description of the attribute. It defaults to an empty string. |
| prefix | `boolean` | Specifies the prefix of the attribute. If this parameter is set, the resolved id of the referenced attribute will have group prefix added to it. It defaults to false. |
| requirement_level | [`RequirementLevel`](#RequirementLevel) | Specifies if the attribute is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the attribute is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the attribute is required. |
| sampling_relevant | Boolean or Null | Specifies if the attribute is (especially) relevant for sampling and thus should be set at span start. It defaults to false. Note: this field is experimental. |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the attribute. Note that, if stability is missing but deprecated is present, it will automatically set the stability to deprecated. If deprecated is present and stability differs from deprecated, this will result in an error. |
| tag | String or Null | Associates a tag ("sub-group") to the attribute. It carries no particular semantic meaning but can be used e.g. for filtering in the markdown generator. |
| tags | Object or Null | A set of tags for the attribute. |
| type | [`AttributeType`](#AttributeType) | Either a string literal denoting the type as a primitive or an array type, a template type or an enum definition. |
| value | [`Value`](#Value) or `null` | The value of the attribute. Note: This is only used in a telemetry schema specification. |

## AttributeLineage

Attribute lineage (at the field level).

| field | type | description |
| --- | --- | --- |
| inherited_fields | `String`[] | A list of fields that are inherited from the source group. |
| locally_overridden_fields | `String`[] | A list of fields that are overridden in the local group. |
| source_group | `String` | The group id where the attribute is coming from. |

## AttributeType

The different types of attributes (specification).

It will have any of the following values: 

- [`PrimitiveOrArrayTypeSpec`](#PrimitiveOrArrayTypeSpec): Primitive or array type.
- [`TemplateTypeSpec`](#TemplateTypeSpec): A template type.
**An Object:** An enum definition type.

| field | type | description |
| --- | --- | --- |
| allow_custom_values | Boolean or Null | Set to false to not accept values other than the specified members. No longer used since semconv 1.27.0. |
| members | [`EnumEntriesSpec`](#EnumEntriesSpec)[] | List of enum entries. |


## BasicRequirementLevelSpec

The different types of basic requirement levels.

It will have one of the following values: 

- `"required"` : A required requirement level.
- `"recommended"` : An optional requirement level.
- `"opt_in"` : An opt-in requirement level.

## Deprecated

The different ways to deprecate an attribute, a metric, ...

It will have one of the following values: 

**An Object:** The telemetry object containing the deprecated field has been renamed to an existing or a new telemetry object.

| field | type | description |
| --- | --- | --- |
| note | `String` | The note to provide more context about the deprecation. |
| reason | `String` | |
| renamed_to | `String` | The new name of the telemetry object. |

**An Object:** The telemetry object containing the deprecated field has been obsoleted because it no longer exists and has no valid replacement.

The `brief` field should contain the reason why the field has been obsoleted.

| field | type | description |
| --- | --- | --- |
| note | `String` | The note to provide more context about the deprecation. |
| reason | `String` | |

**An Object:** The telemetry object containing the deprecated field has been deprecated for complex reasons (split, merge, ...) which are currently not precisely defined in the supported deprecation reasons.

The `brief` field should contain the reason for this uncategorized deprecation.

| field | type | description |
| --- | --- | --- |
| note | `String` | The note to provide more context about the deprecation. |
| reason | `String` | |


## EnumEntriesSpec

Possible enum entries.

| field | type | description |
| --- | --- | --- |
| brief | String or Null | Brief description of the enum entry value. It defaults to the value of id. |
| deprecated | String or Null | Deprecation note. |
| id | `String` | String that uniquely identifies the enum entry. |
| note | String or Null | Longer description. It defaults to an empty string. |
| stability | [`Stability`](#Stability) or `null` | Stability of this enum value. |
| value | [`ValueSpec`](#ValueSpec) | String, int, or boolean; value of the enum entry. |

## Examples

The different types of examples.

It will have any of the following values: 

- `boolean`: A boolean example.
- `int`: A integer example.
- `double`: A double example.
- `String`: A string example.
- [`ValueSpec`](#ValueSpec): A any example.
- `Object`[]: A array of integers example.
- `Object`[]: A array of doubles example.
- `Object`[]: A array of bools example.
- `Object`[]: A array of strings example.
- `Object`[]: A array of anys example.
- `Object`[]: List of arrays of integers example.
- `Object`[]: List of arrays of doubles example.
- `Object`[]: List of arrays of bools example.
- `Object`[]: List of arrays of strings example.

## GroupLineage

Group lineage.

| field | type | description |
| --- | --- | --- |
| attributes | `Object` | The lineage per attribute.

Note: Use a BTreeMap to ensure a deterministic order of attributes. This is important to keep unit tests stable. |
| provenance | [`Provenance`](#Provenance) | The provenance of the source file where the group is defined. |

## GroupType

The different types of groups (specification).

It will have one of the following values: 

- `"attribute_group"` : Attribute group (attribute_group type) defines a set of attributes that can be declared once and referenced by semantic conventions for different signals, for example spans and logs. Attribute groups don't have any specific fields and follow the general semconv semantics.
- `"span"` : Span semantic convention.
- `"event"` : Event semantic convention.
- `"metric"` : Metric semantic convention.
- `"metric_group"` : The metric group semconv is a group where related metric attributes can be defined and then referenced from other metric groups using ref.
- `"entity"` : Entity semantic convention.
- `"scope"` : Scope.
- `"undefined"` : Undefined group type.

## InstrumentSpec

The type of the metric.

It will have one of the following values: 

- `"updowncounter"` : An up-down counter metric.
- `"counter"` : A counter metric.
- `"gauge"` : A gauge metric.
- `"histogram"` : A histogram metric.

## PrimitiveOrArrayTypeSpec

Primitive or array types.

It will have one of the following values: 

- `"boolean"` : A boolean attribute.
- `"int"` : A integer attribute (signed 64 bit integer).
- `"double"` : A double attribute (double precision floating point (IEEE 754-1985)).
- `"string"` : A string attribute.
- `"any"` : An any type attribute (accepts any valid value).
- `"string[]"` : An array of strings attribute.
- `"int[]"` : An array of integer attribute.
- `"double[]"` : An array of double attribute.
- `"boolean[]"` : An array of boolean attribute.

## Provenance

The provenance a semantic convention specification file.

| field | type | description |
| --- | --- | --- |
| path | `String` | The path to the specification file. |
| registry_id | `String` | The registry id containing the specification file. A registry id is an identifier defined in the `registry_manifest.yaml` file. |

## RequirementLevel

The different requirement level specifications.

It will have any of the following values: 

- [`BasicRequirementLevelSpec`](#BasicRequirementLevelSpec): A basic requirement level.
**An Object:** A conditional requirement level.

| field | type | description |
| --- | --- | --- |
| conditionally_required | `String` | The description of the condition. |

**An Object:** A recommended requirement level.

| field | type | description |
| --- | --- | --- |
| recommended | `String` | The description of the recommendation. |

**An Object:** An opt in requirement level.

| field | type | description |
| --- | --- | --- |
| opt_in | `String` | The description of the recommendation. |


## ResolvedGroup

Resolved group specification used in the context of the template engine.

| field | type | description |
| --- | --- | --- |
| attributes | [`Attribute`](#Attribute)[] | List of attributes that belong to the semantic convention. |
| body | [`AnyValueSpec`](#AnyValueSpec) or `null` | The body specification used for event semantic conventions. |
| brief | `String` | A brief description of the semantic convention. |
| deprecated | [`Deprecated`](#Deprecated) or `null` | Specifies if the semantic convention is deprecated. The string provided as `description` MUST specify why it's deprecated and/or what to use instead. See also stability. |
| display_name | String or Null | The readable name for attribute groups used when generating registry tables. |
| entity_associations | `String`[] | The associated entities of this group. |
| events | `String`[] | List of strings that specify the ids of event semantic conventions associated with this span semantic convention. Note: only valid if type is span |
| extends | String or Null | Reference another semantic convention id. It inherits all attributes defined in the specified semantic convention. |
| id | `String` | The id that uniquely identifies the semantic convention. |
| instrument | [`InstrumentSpec`](#InstrumentSpec) or `null` | The instrument type that should be used to record the metric. Note that the semantic conventions must be written using the names of the synchronous instrument types (counter, gauge, updowncounter and histogram). For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types). Note: This field is required if type is metric. |
| lineage | [`GroupLineage`](#GroupLineage) or `null` | The lineage of the group. |
| metric_name | String or Null | The metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model). Note: This field is required if type is metric. |
| name | String or Null | The name of the event. If not specified, the prefix is used. If prefix is empty (or unspecified), name is required. |
| note | `String` | A more elaborate description of the semantic convention. It defaults to an empty string. |
| prefix | `String` | Prefix for the attributes for this semantic convention. It defaults to an empty string. |
| span_kind | [`SpanKindSpec`](#SpanKindSpec) or `null` | Specifies the kind of the span. Note: only valid if type is span |
| stability | [`Stability`](#Stability) or `null` | Specifies the stability of the semantic convention. Note that, if stability is missing but deprecated is present, it will automatically set the stability to deprecated. If deprecated is present and stability differs from deprecated, this will result in an error. |
| type | [`GroupType`](#GroupType) | The type of the group including the specific fields for each type. |
| unit | String or Null | The unit in which the metric is measured, which should adhere to the [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units). Note: This field is required if type is metric. |

## ResolvedRegistry

A resolved semantic convention registry used in the context of the template and policy engines.

| field | type | description |
| --- | --- | --- |
| groups | [`ResolvedGroup`](#ResolvedGroup)[] | A list of semantic convention groups. |
| registry_url | `String` | The semantic convention registry url. |

## SpanKindSpec

The span kind.

It will have one of the following values: 

- `"internal"` : An internal span.
- `"client"` : A client span.
- `"server"` : A server span.
- `"producer"` : A producer span.
- `"consumer"` : A consumer span.

## Stability

The level of stability for a definition. Defined in [OTEP-232](https://github.com/open-telemetry/oteps/blob/main/text/0232-maturity-of-otel.md)

It will have one of the following values: 

- `"deprecated"` : A deprecated definition.
- `"stable"` : A stable definition.
- `"development"` : A definition in development. Formally known as experimental.
- `"alpha"` : An alpha definition.
- `"beta"` : A beta definition.
- `"release_candidate"` : A release candidate definition.

## TemplateTypeSpec

Template types.

It will have one of the following values: 

- `"template[boolean]"` : A boolean attribute.
- `"template[int]"` : A integer attribute.
- `"template[double]"` : A double attribute.
- `"template[string]"` : A string attribute.
- `"template[any]"` : A any attribute.
- `"template[string[]]"` : An array of strings attribute.
- `"template[int[]]"` : An array of integer attribute.
- `"template[double[]]"` : An array of double attribute.
- `"template[boolean[]]"` : An array of boolean attribute.

## Value

The different types of values.

It will have one of the following values: 

**An Object:** A integer value.

| field | type | description |
| --- | --- | --- |
| type | `String` | |
| value | `int` | The value |

**An Object:** A double value.

| field | type | description |
| --- | --- | --- |
| type | `String` | |
| value | `double` | The value |

**An Object:** A string value.

| field | type | description |
| --- | --- | --- |
| type | `String` | |
| value | `String` | The value |


## ValueSpec

The different types of values.

It will have any of the following values: 

- `int`: A integer value.
- `double`: A double value.
- `String`: A string value.
- `boolean`: A boolean value.

## YamlValue

Type: `null` or `boolean` or `Object` or `Object`[] or `double` or `String`

