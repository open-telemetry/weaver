package live_check_advice

import rego.v1

# Reports the `acme.source` annotation of the definition the checker resolved
# for an attribute, so a test can see which definition won.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.attribute
    source := input.registry_attribute.annotations.acme.source
    advice_type := "annotation_source"
    advice_level := "information"
    advice_context := {
        "attribute_key": input.sample.attribute.name,
        "source": source,
    }
    message := sprintf("Attribute '%s' resolved from '%s'.", [input.sample.attribute.name, source])
}

make_advice(advice_type, advice_level, advice_context, message) := {
    "type": "advice",
    "advice_type": advice_type,
    "advice_level": advice_level,
    "advice_context": advice_context,
    "message": message,
}
