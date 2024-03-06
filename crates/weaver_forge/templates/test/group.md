{%- set file_name = group.id | file_name -%}
{{- template.set_file_name("group/" ~ file_name ~ ".md") -}}

# {{ group.typed_group.type }}  `{{ group.id }}`

## Brief

{{ group.brief }} 

## Attributes

{%- for attribute in group.attributes %}
### Attribute `{{ attribute.name }}`

{% if attribute.tag -%}
tag: {{ attribute.tag }}
{%- endif %}

{{ attribute.brief }}

{%- endfor %}

## Provenance

Source: {{ group.lineage.provenance }}

{{ debug() }}

{%- for item in group.lineage.attributes -%}
item: {{ group.lineage.attributes[item] }}
{% endfor -%}

