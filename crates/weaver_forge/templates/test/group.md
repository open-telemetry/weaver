{%- set file_name = id | file_name -%}
{{- template.set_file_name("group/" ~ file_name ~ ".md") -}}
# {{ typed_group.type }}  `{{ id }}`

## Brief

{{ brief }} 

## Provenance

{{- debug() -}}

{{ lineage.attributes }}

/*
{%- for key, value in lineage.attributes -%}
{{- key -}}
{%- endfor -%}
*/
