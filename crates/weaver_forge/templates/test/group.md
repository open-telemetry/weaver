{%- set file_name = id | file_name -%}
{{- template.set_file_name("group/" ~ file_name ~ ".md") -}}
# Group  `{{ id }}`

file name: {{ id | file_name }}
function name: {{ id | function_name }}
arg_name: {{ id | arg_name }}
struct_name: {{ id | struct_name }}
field_name: {{ id | field_name }}

