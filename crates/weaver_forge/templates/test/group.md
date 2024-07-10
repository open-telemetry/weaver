{%- set file_name = ctx.id | snake_case -%}
{{- template.set_file_name("group/" ~ file_name ~ ".md") -}}

# Group `{{ ctx.id }}` ({{ ctx.type }})

## Brief

{{ ctx.brief | trim }}

prefix: {{ ctx.prefix }}

{% if ctx.type == "event" -%}
{%- set event = ctx -%}
{% include "body.j2" -%}
{% endif -%}

## Attributes

{% for attribute in ctx.attributes %}
### Attribute `{{ attribute.name }}`

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

## Lineage

Source file: {{ ctx.lineage.source_file | replace("\\", "/") }}

{% for item in ctx.lineage.attributes -%}
attribute: {{ item.id }}
  - source group: {{ item.source_group }}
  - inherited fields: {{ item.inherited_fields | join(", ") }}
  - locally overridden fields: {{ item.locally_overridden_fields | join(", ") }}

{% endfor -%}