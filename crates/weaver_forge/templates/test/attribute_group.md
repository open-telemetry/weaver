{%- set file_name = ctx.id | file_name -%}
{{- template.set_file_name("attribute_group/" ~ file_name ~ ".md") -}}

## Group `{{ ctx.id | split_id | list | join("_") }}` ({{ ctx.type }})

### Brief

{{ ctx.brief | trim }}

prefix: {{ ctx.prefix }}

### Attributes

{% for attribute in ctx.attributes %}
#### Attribute `{{ attribute.name }}`

{{ attribute.brief | trim }}

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
