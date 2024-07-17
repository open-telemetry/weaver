# Semantic Convention Registry

Url:{{ registry_url }}

## Attribute Groups

{% for group in ctx.groups %}
{% if group.type == "attribute_group" %}
- [{{ group.id }}](attribute_group/{{ group.id | snake_case }}.md)
{% endif %}
{% endfor %}

## Events

{% for group in ctx.groups %}
{% if group.type == "event" %}
- [{{ group.id }}](event/{{ group.id | snake_case }}.md)
{% endif %}
{% endfor %}

## Metrics

{% for group in ctx.groups %}
{% if group.type == "metric" %}
- [{{ group.id }}](metric/{{ group.id | snake_case }}.md)
{% endif %}
{% endfor %}

## Metric Groups
{% for group in ctx.groups %}
{% if group.type == "metric_group" %}
- [{{ group.id }}](metric_group/{{ group.id | snake_case }}.md)
{% endif %}
{% endfor %}

## Resource

{% for group in ctx.groups %}
{% if group.type == "resource" %}
- [{{ group.id }}](resource/{{ group.id | snake_case }}.md)
{% endif %}
{% endfor %}

## Scope
{% for group in ctx.groups %}
{% if group.type == "scope" %}
- [{{ group.id }}](scope/{{ group.id | snake_case }}.md)
{% endif %}
{% endfor %}

## Span

{% for group in ctx.groups %}
{% if group.type == "span" %}
- [{{ group.id }}](span/{{ group.id | snake_case }}.md)
{% endif %}
{% endfor %}
