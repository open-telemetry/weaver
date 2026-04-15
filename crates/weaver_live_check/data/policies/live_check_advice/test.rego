package live_check_advice

import rego.v1

# checks attribute name contains the word "test"
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.attribute
    contains(input.sample.attribute.name, "test")
    advice_type := "contains_test"
    advice_level := "violation"
    advice_context := {
        "attribute_key": input.sample.attribute.name
    }
    message := sprintf("Attribute name must not contain 'test', but was '%s'", [input.sample.attribute.name])
}

# checks span name contains the word "test"
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.span
    contains(input.sample.span.name, "test")
    advice_type := "contains_test"
    advice_level := "violation"
    advice_context := {
        "span_name": input.sample.span.name
    }
    message :=  sprintf("Span name must not contain 'test', but was '%s'", [input.sample.span.name])
}

# checks span status message contains the word "test"
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.span
    contains(input.sample.span.status.message, "test")
    advice_type := "contains_test_in_status"
    advice_level := "violation"
    advice_context := {
        "span_status_message": input.sample.span.status.message
    }
    message :=  sprintf("Span status message must not contain 'test', but was '%s'", [input.sample.span.status.message])
}

# checks span_event name contains the word "test"
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.span_event
    contains(input.sample.span_event.name, "test")
    advice_type := "contains_test"
    advice_level := "violation"
    advice_context := {
        "span_event_name": input.sample.span_event.name
    }
    message :=  sprintf("Span event name must not contain 'test', but was '%s'", [input.sample.span_event.name])
}

# This example shows how to use the registry_group provided in the input.
# If the metric's unit is "By" the value in this data-point must be an integer.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.number_data_point
    input.registry_group.unit == "By"
    input.sample.number_data_point.value != floor(input.sample.number_data_point.value) # not a good type check, but serves as an example
    advice_context := {
        "data_point_value": input.sample.number_data_point.value
    }
    advice_type := "invalid_data_point_value"
    advice_level := "violation"
    message := sprintf("Metric with unit 'By' must have an integer value, but was '%v'", [input.sample.number_data_point.value])
}

# As above, but for exemplars which are nested two levels deep.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.exemplar
    input.registry_group.unit == "s"
    input.sample.exemplar.value < 1.0
    advice_type := "low_value"
    advice_level := "information"
    advice_context := {
        "exemplar_value": input.sample.exemplar.value
    }
    message := sprintf("This is a low number of seconds: %v", [input.sample.exemplar.value])
}

make_advice(advice_type, advice_level, advice_context, message) := {
    "type": "advice",
    "advice_type": advice_type,
    "advice_level": advice_level,
    "advice_context": advice_context,
    "message": message,
}

# Log with an event_name that we can compare against the matched event in the model.
# If the registry_group is present, check for an annotation that specifies a phrase
# which should be found in the body.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.log
    input.registry_group.annotations.required_phrase
    phrase := input.registry_group.annotations.required_phrase
    not contains(input.sample.log.body, phrase)
    advice_type := "required_phrase_missing"
    advice_level := "violation"
    advice_context := {
        "event_name": input.sample.log.event_name,
        "required_phrase": phrase
    }
    message := sprintf("Event '%s' body does not contain the required phrase: '%s'", [input.sample.log.event_name, phrase])
}

# Log with an empty event.name that we can still have policies against.
# Must not have an empty body.
deny contains make_advice(advice_type, advice_level, advice_context, message) if {
    input.sample.log
    input.sample.log.event_name == ""
    input.sample.log.body == ""
    advice_type := "empty_body"
    advice_level := "violation"
    advice_context := {}
    message := "Logs must not have an empty body."
}