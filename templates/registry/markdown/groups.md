# Semantic Convention Groups

{% for group in ctx %}
## Group `{{ group.id }}` ({{ group.type }})

### Brief

{{ group.brief | trim }}

prefix: {{ group.prefix }}

### Attributes

{% for attribute in group.attributes %}
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
{% if group.lineage.attributes %}
{% set attr_lineage = group.lineage.attributes[attribute.name] %}
{% if attr_lineage %}
Lineage:
- source group: {{ attr_lineage.source_group }}
- inherited fields: {{ attr_lineage.inherited_fields | join(", ") }}
- locally overridden fields: {{ attr_lineage.locally_overridden_fields | join(", ") }}
{% endif %}
{% endif %}
{% endfor %}
{% endfor %}
