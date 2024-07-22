{%- set file_name = ctx.root_namespace | snake_case -%}
{{- template.set_file_name("metric/" ~ file_name ~ ".md") -}}

## Metrics Namespace `{{ ctx.root_namespace }}` 

{% for metric in ctx.metrics %}
## Metric `{{ metric.metric_name }}`

Instrument: {{ metric.instrument }}
Unit: {{ metric.unit }}
Stability: {{ metric.stability }}

### Attributes

{% for attribute in metric.attributes %}
#### Attribute `{{ attribute.name }}`

{{ attribute.brief }}

{% if attribute.note %}
{{ attribute.note | trim }}
{% endif %}

{%- if attribute.requirement_level == "required" %}
- Requirement Level: Required
  {%- elif attribute.requirement_level.conditionally_required %}
- Requirement Level: Conditionally Required - {{ attribute.requirement_level.conditionally_required }}
  {%- elif attribute.requirement_level == "recommended" %}
- Requirement Level: Recommended
  {%- else %}
- Requirement Level: Optional
  {%- endif %}
  {% if attribute.tag %}
- Tag: {{ attribute.tag }}
  {% endif %}
  {%- include "attribute_type.j2" %}
  {%- include "examples.j2" -%}
  {%- if attribute.sampling_relevant %}
- Sampling relevant: {{ attribute.sampling_relevant }}
  {%- endif %}
  {%- if attribute.deprecated %}
- Deprecated: {{ attribute.deprecated }}
  {%- endif %}
  {% if attribute.stability %}
- Stability: {{ attribute.stability | capitalize }}
  {% endif %}
  {% endfor %}
  {% endfor %}