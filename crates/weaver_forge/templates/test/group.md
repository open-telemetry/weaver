{%- set file_name = group.id | file_name -%}
{{- template.set_file_name("group/" ~ file_name ~ ".md") -}}

# {{ group.type }}  `{{ group.id }}`

## Brief

{{ group.brief }}
## Attributes

{% for attribute in group.attributes -%}
### Attribute `{{ attribute.name }}`

Requirement level: {{ attribute.requirement_level }}

{% if attribute.tag -%}
Tag: {{ attribute.tag }}
{%- endif %}

Brief: {{ attribute.brief }}

Type: {{ attribute.type }}

{% if attribute.note -%}
Note: {{ attribute.note }}
{%- endif %}

{% if attribute.stability -%}
Stability: {{ attribute.stability }}
{%- endif %}
{% endfor %}

## Provenance

Source: {{ group.lineage.provenance }}

{{ debug() }}

{%- for item in group.lineage.attributes -%}
item: {{ group.lineage.attributes[item] }}
{% endfor -%}

