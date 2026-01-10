<!-- semconv registry.spans.trace.test -->
trace.test param
<!-- endsemconv -->

<!-- semconv refinements.metrics.test(metric_table) -->
test.metric param
<!-- endsemconv -->

<!-- semconv refinements.events.test -->
test.event param
<!-- endsemconv -->

<!-- semconv registry.entities.test.entity -->
test.entity param
<!-- endsemconv -->

<!-- semconv registry.attribute_groups.test.common -->
test.common
<!-- endsemconv -->

<!-- weaver template:custom.j2 . -->
Custom Snippet Name
<!-- endweaver -->

<!-- weaver template:registry.md.j2 {value:.registry_url} -->
todo/1.0.0
<!-- endweaver -->

<!-- weaver template:registry.md.j2 { value: .registry.metrics[] | select(.name == "test.metric") | .unit } -->
{1}
<!-- endweaver -->
