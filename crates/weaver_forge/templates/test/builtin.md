## flatten
{%- set test = [["one", "two"], ["three"]] | flatten -%}
{% for item in test %}
- {{item}}
{%- endfor -%}