{%- set file_name = ctx.group_namespace | snake_case -%}
{{- template.set_file_name("event/" ~ file_name ~ ".md") -}}

## Events Namespace `{{ ctx.group_namespace }}`

{% for event in ctx.events %}
## Event `{{ event.id }}`

Note: {{ event.note }}
Brief: {{ event.brief }}
Requirement level: {{ event.requirement_level }}
Stability: {{ event.stability }}

### Body Fields

{% if event.body -%}
{% for bodyField in event.body.fields -%}
#### Field `{{ bodyField.name }}`

{{ bodyField.brief }}

{%- if bodyField.note %}
{{ bodyField.note | trim }}
{% endif %}

{%- if bodyField.requirement_level == "required" %}
- Requirement Level: Required
{%- elif bodyField.requirement_level.conditionally_required %}
- Requirement Level: Conditionally Required - {{ bodyField.requirement_level.conditionally_required }}
{%- elif bodyField.requirement_level == "recommended" %}
- Requirement Level: Recommended
{%- else %}
- Requirement Level: Optional
{%- endif %}

{%- if bodyField.type is mapping %}
- Type: Enum [{{ bodyField.type.members | map(attribute="value") | join(", ") | trim }}]
{%- else %}
- Type: {{ bodyField.type }}
{%- endif %}

{%- if bodyField.examples %}
{%- if bodyField.examples is sequence %}
- Examples: {{ bodyField.examples | pprint }}
{%- else %}
- Examples: {{ bodyField.examples }}
{%- endif %}
{%- endif %}

{%- if bodyField.deprecated %}
- Deprecated: {{ bodyField.deprecated }}
{%- endif %}

{%- if bodyField.stability %}
- Stability: {{ bodyField.stability | capitalize }}
{%- endif %}

{% endfor -%}
{%- else -%}

No event body defined.

{%- endif -%}

### Attributes

{% for attribute in event.attributes %}
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