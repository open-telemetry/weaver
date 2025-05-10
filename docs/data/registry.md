# Template DataModels

## AnyValueSpec

The AnyValueTypeSpec is a specification of a value that can be of any type.

One of the following: 

A boolean attribute.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

A integer attribute (signed 64 bit integer).

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

A double attribute (double precision floating point (IEEE 754-1985)).

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

A string attribute.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

An array of strings attribute.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

An array of integer attribute.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

An array of double attribute.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

An array of boolean attribute.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

The value type is a map of key, value pairs

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `fields` | `AnyValueSpec[]` | The collection of key, values where the value is an `AnyValueSpec` |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

The value type is a map of key, value pairs

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `fields` | `AnyValueSpec[]` | The collection of key, values where the value is an `AnyValueSpec` |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

The value type will just be a bytes.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

The value type is not specified.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |

An enum definition type.

| field | type | description |
| --- | --- | --- |
| `brief` | `string` | A brief description of the value |
| `examples` | `Examples | null` | Sequence of examples for the value or single example value. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `members` | `EnumEntriesSpec[]` | List of enum entries. |
| `note` | `string` | A more elaborate description of the value. It defaults to an empty string. |
| `requirement_level` | `RequirementLevel` | Specifies if the field is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the field is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the field is required. |
| `stability` | `Stability | null` | Specifies the stability of the value. |
| `type` | `string` | |


## Attribute

An attribute definition.

| field | type | description |
| --- | --- | --- |
| `annotations` | `Object | Null` | Annotations for the group. |
| `brief` | `string` | A brief description of the attribute. |
| `deprecated` | `Deprecated | null` | Specifies if the attribute is deprecated. |
| `examples` | `Examples | null` | Sequence of example values for the attribute or single example value. They are required only for string and string array attributes. Example values must be of the same type of the attribute. If only a single example is provided, it can directly be reported without encapsulating it into a sequence/dictionary. |
| `name` | `string` | Attribute name. |
| `note` | `string` | A more elaborate description of the attribute. It defaults to an empty string. |
| `prefix` | `boolean` | Specifies the prefix of the attribute. If this parameter is set, the resolved id of the referenced attribute will have group prefix added to it. It defaults to false. |
| `requirement_level` | `RequirementLevel` | Specifies if the attribute is mandatory. Can be "required", "conditionally_required", "recommended" or "opt_in". When omitted, the attribute is "recommended". When set to "conditionally_required", the string provided as <condition> MUST specify the conditions under which the attribute is required. |
| `sampling_relevant` | `Boolean | Null` | Specifies if the attribute is (especially) relevant for sampling and thus should be set at span start. It defaults to false. Note: this field is experimental. |
| `stability` | `Stability | null` | Specifies the stability of the attribute. Note that, if stability is missing but deprecated is present, it will automatically set the stability to deprecated. If deprecated is present and stability differs from deprecated, this will result in an error. |
| `tag` | `String | Null` | Associates a tag ("sub-group") to the attribute. It carries no particular semantic meaning but can be used e.g. for filtering in the markdown generator. |
| `tags` | `Object | Null` | A set of tags for the attribute. |
| `type` | `AttributeType` | Either a string literal denoting the type as a primitive or an array type, a template type or an enum definition. |
| `value` | `Value | null` | The value of the attribute. Note: This is only used in a telemetry schema specification. |

## AttributeLineage

Attribute lineage (at the field level).

| field | type | description |
| --- | --- | --- |
| `inherited_fields` | `string[]` | A list of fields that are inherited from the source group. |
| `locally_overridden_fields` | `string[]` | A list of fields that are overridden in the local group. |
| `source_group` | `string` | The group id where the attribute is coming from. |

## AttributeType

The different types of attributes (specification).

todo: SubschemaValidation { all_of: None, any_of: Some([Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Primitive or array type."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: None, format: None, enum_values: None, const_value: None, subschemas: Some(SubschemaValidation { all_of: Some([Object(SchemaObject { metadata: None, instance_type: None, format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: Some("#/definitions/PrimitiveOrArrayTypeSpec"), extensions: {} })]), any_of: None, one_of: None, not: None, if_schema: None, then_schema: None, else_schema: None }), number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A template type."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: None, format: None, enum_values: None, const_value: None, subschemas: Some(SubschemaValidation { all_of: Some([Object(SchemaObject { metadata: None, instance_type: None, format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: Some("#/definitions/TemplateTypeSpec"), extensions: {} })]), any_of: None, one_of: None, not: None, if_schema: None, then_schema: None, else_schema: None }), number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An enum definition type."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Object)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: Some(ObjectValidation { max_properties: None, min_properties: None, required: {"members"}, properties: {"allow_custom_values": Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Set to false to not accept values other than the specified members. No longer used since semconv 1.27.0."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Vec([Boolean, Null])), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }), "members": Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("List of enum entries."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: None, format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: Some("#/definitions/EnumEntriesSpec"), extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} })}, pattern_properties: {}, additional_properties: None, property_names: None }), reference: None, extensions: {} })]), one_of: None, not: None, if_schema: None, then_schema: None, else_schema: None }

## BasicRequirementLevelSpec

The different types of basic requirement levels.

One of the following: 

SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A required requirement level."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("required")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An optional requirement level."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("recommended")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An opt-in requirement level."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("opt_in")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }

## Deprecated

The different ways to deprecate an attribute, a metric, ...

One of the following: 

The telemetry object containing the deprecated field has been renamed to an existing or a new telemetry object.

| field | type | description |
| --- | --- | --- |
| `note` | `string` | The note to provide more context about the deprecation. |
| `reason` | `string` | |
| `renamed_to` | `string` | The new name of the telemetry object. |

The telemetry object containing the deprecated field has been obsoleted because it no longer exists and has no valid replacement.

The `brief` field should contain the reason why the field has been obsoleted.

| field | type | description |
| --- | --- | --- |
| `note` | `string` | The note to provide more context about the deprecation. |
| `reason` | `string` | |

The telemetry object containing the deprecated field has been deprecated for complex reasons (split, merge, ...) which are currently not precisely defined in the supported deprecation reasons.

The `brief` field should contain the reason for this uncategorized deprecation.

| field | type | description |
| --- | --- | --- |
| `note` | `string` | The note to provide more context about the deprecation. |
| `reason` | `string` | |


## EnumEntriesSpec

Possible enum entries.

| field | type | description |
| --- | --- | --- |
| `brief` | `String | Null` | Brief description of the enum entry value. It defaults to the value of id. |
| `deprecated` | `String | Null` | Deprecation note. |
| `id` | `string` | String that uniquely identifies the enum entry. |
| `note` | `String | Null` | Longer description. It defaults to an empty string. |
| `stability` | `Stability | null` | Stability of this enum value. |
| `value` | `ValueSpec` | String, int, or boolean; value of the enum entry. |

## Examples

The different types of examples.

todo: SubschemaValidation { all_of: None, any_of: Some([Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A boolean example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Boolean)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A integer example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Integer)), format: Some("int64"), enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A double example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Number)), format: Some("double"), enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A string example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A any example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: None, format: None, enum_values: None, const_value: None, subschemas: Some(SubschemaValidation { all_of: Some([Object(SchemaObject { metadata: None, instance_type: None, format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: Some("#/definitions/ValueSpec"), extensions: {} })]), any_of: None, one_of: None, not: None, if_schema: None, then_schema: None, else_schema: None }), number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A array of integers example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Integer)), format: Some("int64"), enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A array of doubles example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Number)), format: Some("double"), enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A array of bools example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Boolean)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A array of strings example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(String)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A array of anys example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: None, format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: Some("#/definitions/ValueSpec"), extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("List of arrays of integers example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Integer)), format: Some("int64"), enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("List of arrays of doubles example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Number)), format: Some("double"), enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("List of arrays of bools example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Boolean)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("List of arrays of strings example."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(Array)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: Some(ArrayValidation { items: Some(Single(Object(SchemaObject { metadata: None, instance_type: Some(Single(String)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} }))), additional_items: None, max_items: None, min_items: None, unique_items: None, contains: None }), object: None, reference: None, extensions: {} })]), one_of: None, not: None, if_schema: None, then_schema: None, else_schema: None }

## GroupLineage

Group lineage.

| field | type | description |
| --- | --- | --- |
| `attributes` | `Object` | The lineage per attribute.

Note: Use a BTreeMap to ensure a deterministic order of attributes. This is important to keep unit tests stable. |
| `provenance` | `Provenance` | The provenance of the source file where the group is defined. |

## GroupType

The different types of groups (specification).

One of the following: 

SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Attribute group (attribute_group type) defines a set of attributes that can be declared once and referenced by semantic conventions for different signals, for example spans and logs. Attribute groups don't have any specific fields and follow the general semconv semantics."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("attribute_group")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Span semantic convention."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("span")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Event semantic convention."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("event")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Metric semantic convention."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("metric")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("The metric group semconv is a group where related metric attributes can be defined and then referenced from other metric groups using ref."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("metric_group")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Entity semantic convention."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("entity")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Scope."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("scope")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("Undefined group type."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("undefined")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }

## InstrumentSpec

The type of the metric.

One of the following: 

SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An up-down counter metric."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("updowncounter")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A counter metric."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("counter")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A gauge metric."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("gauge")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A histogram metric."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("histogram")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }

## PrimitiveOrArrayTypeSpec

Primitive or array types.

One of the following: 

SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A boolean attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("boolean")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A integer attribute (signed 64 bit integer)."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("int")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A double attribute (double precision floating point (IEEE 754-1985))."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("double")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A string attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("string")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An any type attribute (accepts any valid value)."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("any")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An array of strings attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("string[]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An array of integer attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("int[]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An array of double attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("double[]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An array of boolean attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("boolean[]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }

## Provenance

The provenance a semantic convention specification file.

| field | type | description |
| --- | --- | --- |
| `path` | `string` | The path to the specification file. |
| `registry_id` | `string` | The registry id containing the specification file. A registry id is an identifier defined in the `registry_manifest.yaml` file. |

## RequirementLevel

The different requirement level specifications.

todo: SubschemaValidation { all_of: None, any_of: Some([Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A basic requirement level."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: None, format: None, enum_values: None, const_value: None, subschemas: Some(SubschemaValidation { all_of: Some([Object(SchemaObject { metadata: None, instance_type: None, format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: Some("#/definitions/BasicRequirementLevelSpec"), extensions: {} })]), any_of: None, one_of: None, not: None, if_schema: None, then_schema: None, else_schema: None }), number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A conditional requirement level."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Object)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: Some(ObjectValidation { max_properties: None, min_properties: None, required: {"conditionally_required"}, properties: {"conditionally_required": Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("The description of the condition."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} })}, pattern_properties: {}, additional_properties: None, property_names: None }), reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A recommended requirement level."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Object)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: Some(ObjectValidation { max_properties: None, min_properties: None, required: {"recommended"}, properties: {"recommended": Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("The description of the recommendation."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} })}, pattern_properties: {}, additional_properties: None, property_names: None }), reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An opt in requirement level."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Object)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: Some(ObjectValidation { max_properties: None, min_properties: None, required: {"opt_in"}, properties: {"opt_in": Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("The description of the recommendation."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} })}, pattern_properties: {}, additional_properties: None, property_names: None }), reference: None, extensions: {} })]), one_of: None, not: None, if_schema: None, then_schema: None, else_schema: None }

## ResolvedGroup

Resolved group specification used in the context of the template engine.

| field | type | description |
| --- | --- | --- |
| `attributes` | `Attribute[]` | List of attributes that belong to the semantic convention. |
| `body` | `AnyValueSpec | null` | The body specification used for event semantic conventions. |
| `brief` | `string` | A brief description of the semantic convention. |
| `deprecated` | `Deprecated | null` | Specifies if the semantic convention is deprecated. The string provided as `description` MUST specify why it's deprecated and/or what to use instead. See also stability. |
| `display_name` | `String | Null` | The readable name for attribute groups used when generating registry tables. |
| `entity_associations` | `string[]` | The associated entities of this group. |
| `events` | `string[]` | List of strings that specify the ids of event semantic conventions associated with this span semantic convention. Note: only valid if type is span |
| `extends` | `String | Null` | Reference another semantic convention id. It inherits all attributes defined in the specified semantic convention. |
| `id` | `string` | The id that uniquely identifies the semantic convention. |
| `instrument` | `InstrumentSpec | null` | The instrument type that should be used to record the metric. Note that the semantic conventions must be written using the names of the synchronous instrument types (counter, gauge, updowncounter and histogram). For more details: [Metrics semantic conventions - Instrument types](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-types). Note: This field is required if type is metric. |
| `lineage` | `GroupLineage | null` | The lineage of the group. |
| `metric_name` | `String | Null` | The metric name as described by the [OpenTelemetry Specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/metrics/data-model.md#timeseries-model). Note: This field is required if type is metric. |
| `name` | `String | Null` | The name of the event. If not specified, the prefix is used. If prefix is empty (or unspecified), name is required. |
| `note` | `string` | A more elaborate description of the semantic convention. It defaults to an empty string. |
| `prefix` | `string` | Prefix for the attributes for this semantic convention. It defaults to an empty string. |
| `span_kind` | `SpanKindSpec | null` | Specifies the kind of the span. Note: only valid if type is span |
| `stability` | `Stability | null` | Specifies the stability of the semantic convention. Note that, if stability is missing but deprecated is present, it will automatically set the stability to deprecated. If deprecated is present and stability differs from deprecated, this will result in an error. |
| `type` | `GroupType` | The type of the group including the specific fields for each type. |
| `unit` | `String | Null` | The unit in which the metric is measured, which should adhere to the [guidelines](https://github.com/open-telemetry/opentelemetry-specification/tree/main/specification/metrics/semantic_conventions#instrument-units). Note: This field is required if type is metric. |

## ResolvedRegistry

A resolved semantic convention registry used in the context of the template and policy engines.

| field | type | description |
| --- | --- | --- |
| `groups` | `ResolvedGroup[]` | A list of semantic convention groups. |
| `registry_url` | `string` | The semantic convention registry url. |

## SpanKindSpec

The span kind.

One of the following: 

SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An internal span."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("internal")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A client span."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("client")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A server span."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("server")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A producer span."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("producer")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A consumer span."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("consumer")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }

## Stability

The level of stability for a definition. Defined in [OTEP-232](https://github.com/open-telemetry/oteps/blob/main/text/0232-maturity-of-otel.md)

One of the following: 

SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A deprecated definition."), default: None, deprecated: true, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("deprecated")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A stable definition."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("stable")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A definition in development. Formally known as experimental."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("development")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An alpha definition."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("alpha")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A beta definition."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("beta")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A release candidate definition."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("release_candidate")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }

## TemplateTypeSpec

Template types.

One of the following: 

SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A boolean attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[boolean]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A integer attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[int]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A double attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[double]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A string attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[string]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A any attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[any]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An array of strings attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[string[]]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An array of integer attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[int[]]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An array of double attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[double[]]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }
SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("An array of boolean attribute."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: Some([String("template[boolean[]]")]), const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }

## Value

The different types of values.

One of the following: 

A integer value.

| field | type | description |
| --- | --- | --- |
| `type` | `string` | |
| `value` | `int` | The value |

A double value.

| field | type | description |
| --- | --- | --- |
| `type` | `string` | |
| `value` | `double` | The value |

A string value.

| field | type | description |
| --- | --- | --- |
| `type` | `string` | |
| `value` | `string` | The value |


## ValueSpec

The different types of values.

todo: SubschemaValidation { all_of: None, any_of: Some([Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A integer value."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Integer)), format: Some("int64"), enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A double value."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Number)), format: Some("double"), enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A string value."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(String)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} }), Object(SchemaObject { metadata: Some(Metadata { id: None, title: None, description: Some("A boolean value."), default: None, deprecated: false, read_only: false, write_only: false, examples: [] }), instance_type: Some(Single(Boolean)), format: None, enum_values: None, const_value: None, subschemas: None, number: None, string: None, array: None, object: None, reference: None, extensions: {} })]), one_of: None, not: None, if_schema: None, then_schema: None, else_schema: None }

## YamlValue

Type: null | boolean | Object | Object[] | double | string

