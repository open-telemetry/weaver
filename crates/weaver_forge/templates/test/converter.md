{%- set text = "this IS an ios device with a nice api!" -%}
text                : {{ text }}
lower_case          : {{ text | lower_case }}
upper_case | acronym: {{ text | upper_case | acronym }}
title_case | acronym: {{ text | title_case | acronym }}
pascal_case         : {{ text | pascal_case }}
camel_case          : {{ text | camel_case }}
snake_case          : {{ text | snake_case }}
screaming_snake_case: {{ text | screaming_snake_case }}
kebab_case | acronym: {{ text | kebab_case | acronym }}
screaming_kebab_case: {{ text | screaming_kebab_case }}