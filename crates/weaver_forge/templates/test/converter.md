{%- set text = "this IS an ios device with a nice api!" -%}
{{ text }}
{{ text | lower_case }}
{{ text | upper_case | acronym }}
{{ text | title_case | acronym }}
{{ text | pascal_case }}
{{ text | camel_case }}
{{ text | snake_case }}
{{ text | screaming_snake_case }}
{{ text | kebab_case | acronym }}
{{ text | screaming_kebab_case }}