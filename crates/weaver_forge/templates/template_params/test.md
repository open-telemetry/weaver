{{- template.set_file_name(params.template_name ~ ".json") -}}
{{ params | tojson(indent=2) }}
