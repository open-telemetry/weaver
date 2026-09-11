# Span Name Templates

Status: Proposal

## Introduction

Semantic conventions currently describe span names via free-form text in the `note` property or in external markdown documentation. This document specifies a formal, declarative mechanism in the Weaver semantic convention schema to define span names using ordered templates.

## Analysis of Existing Conventions

A review of span naming across semantic conventions reveals consistent patterns across domains:

1. **GenAI** (including MCP): Consistently defines span names as an ordered sequence of patterns based on attribute availability:
   - `{gen_ai.operation.name} {gen_ai.request.model}`
   - `{gen_ai.operation.name} {gen_ai.data_source.id}`
   - `{gen_ai.operation.name}`
   - `create_agent {gen_ai.agent.name}`
   - `invoke_agent {gen_ai.agent.name}` -> `invoke_agent`
   - `execute_tool {gen_ai.tool.name} {gen_ai.skill.name} {gen_ai.skill.resource.name}` -> `execute_tool {gen_ai.tool.name} {gen_ai.skill.resource.name}` -> `execute_tool {gen_ai.tool.name} {process.executable.name}` -> `execute_tool {gen_ai.tool.name}`
   - `invoke_workflow {gen_ai.workflow.name}`
   - `plan {gen_ai.agent.name}` -> `plan`
   - `{mcp.method.name} {gen_ai.tool.name}` -> `{mcp.method.name} {gen_ai.prompt.name}` -> `{mcp.method.name}`

2. **HTTP**:
   - Client: `{http.request.method} {url.template}` -> `{http.request.method} {server.address}:{server.port}` -> `{http.request.method} {server.address}` -> `{http.request.method}` -> `HTTP`
   - Server: `{http.request.method} {http.route}` -> `{http.request.method}` -> `HTTP`
   - Sentinel rule: When `{http.request.method}` is `_OTHER` treat it as not provided.

3. **Database**:
   - `{db.query.summary}`
   - `{db.operation.name} {db.collection.name}` -> `{db.operation.name} {db.namespace}` -> `{db.operation.name} {server.address}` -> `{db.operation.name}`
   - `{db.collection.name}` -> `{db.system.name}`

4. **Messaging**:
   - `{messaging.operation.name} {messaging.destination.template}` -> `{messaging.operation.name} {messaging.destination.name}` -> `{messaging.operation.name} {server.address}:{server.port}` -> `{messaging.operation.name}`

5. **RPC, GraphQL, CI/CD, FaaS**:
   - RPC: `{rpc.method}` -> `{rpc.system.name}`
   - GraphQL: `{graphql.operation.type}` -> `GraphQL Operation`
   - CI/CD: `{cicd.pipeline.action.name} {cicd.pipeline.name}` -> `{cicd.pipeline.action.name}`
   - FaaS: `{faas.name}` -> `{faas.invoked_name}`

### Findings

Over 90% of all span naming conventions in OpenTelemetry follow the identical pattern:
1. An ordered list of template strings.
2. Evaluated from top to bottom.
3. The first template whose referenced attributes are all present (non-empty) is selected.
4. A trailing literal template without placeholders acts as a static fallback.
5. `_OTHER` is a specification-wide sentinel indicating an unknown enum member that must never be emitted into a span name. Non-enum attributes (e.g. URLs, addresses) are never checked against `_OTHER`.

---

## Design

### 1. Schema Syntax

The `name` object on a span is extended with `templates`:

```yaml
spans:
  - type: gen_ai.execute_tool.command.internal
    kind: internal
    name:
      templates:
        - "execute_tool {gen_ai.tool.name} {gen_ai.skill.name} {gen_ai.skill.resource.name}"
        - "execute_tool {gen_ai.tool.name} {gen_ai.skill.resource.name}"
        - "execute_tool {gen_ai.tool.name} {process.executable.name}"
        - "execute_tool {gen_ai.tool.name}"
```

And for HTTP client:

```yaml
spans:
  - type: http.client
    kind: client
    name:
      templates:
        - "{http.request.method} {url.template}"
        - "{http.request.method} {server.address}:{server.port}"
        - "{http.request.method} {server.address}"
        - "{http.request.method}"
        - "HTTP"
```

#### Fields:
- **`templates`** (`Vec<String>`, optional): Ordered list of template patterns. A literal template with no placeholders (e.g. `"HTTP"`) placed at the end serves as an unconditional fallback.
- **`note`** (`Option<String>`, optional): Free-form context, explanations, and edge cases. Required if `templates` is omitted.

### 2. Matching and Presence Semantics

When evaluating templates:
1. Templates are checked in order.
2. A placeholder `{attribute.key}` is satisfied if and only if the attribute:
   - Is present in the telemetry payload / attributes map.
   - Has a non-empty value.
   - If the attribute is an enum, is not equal to the sentinel value `_OTHER`.
3. A template matches if **all** of its referenced placeholders are satisfied. A template with no placeholders always matches.
4. If no template matches, evaluation falls back to the span type (or static operation name).

### 3. Code Generation (Jinja / Weaver Forge)

Jinja templates can generate fast runtime span name resolvers. By resolving referenced attributes upfront, generated code avoids repeated lookups and branches directly on local optionals. For enum attributes, `_OTHER` is filtered out. When no template matches, the resolver falls back to the span's `type`.

Example Jinja template generating Rust:

```jinja
{% set all_attrs = span.name.templates | map(attribute="attributes") | flatten | unique %}
pub fn resolve_span_name(attrs: &Attributes) -> String {
    {%- for attr in all_attrs %}
    {%- if attr.is_enum %}
    let {{ attr.name | snake_case }} = attrs.get("{{ attr.name }}").filter(|v| *v != "_OTHER");
    {%- else %}
    let {{ attr.name | snake_case }} = attrs.get("{{ attr.name }}");
    {%- endif %}
    {%- endfor %}

    {% for t in span.name.templates %}
    {%- if t.attributes %}
    if let ({% for attr in t.attributes %}Some({{ attr | snake_case }}){% if not loop.last %}, {% endif %}{% endfor %}) = ({% for attr in t.attributes %}{{ attr | snake_case }}{% if not loop.last %}, {% endif %}{% endfor %}) {
        return format!("{{ t.pattern | replace("{", "{") }}", {% for attr in t.attributes %}{{ attr | snake_case }} = {{ attr | snake_case }}{% if not loop.last %}, {% endif %}{% endfor %});
    }
    {%- else %}
    return "{{ t.pattern }}".to_string();
    {%- endif %}
    {%- endfor %}

    "{{ span.type }}".to_string()
}
```

Generated Rust code for `gen_ai.execute_tool.command.internal` (falling back to span type):

```rust
pub fn resolve_span_name(attrs: &Attributes) -> String {
    let gen_ai_tool_name = attrs.get("gen_ai.tool.name");
    let gen_ai_skill_name = attrs.get("gen_ai.skill.name");
    let gen_ai_skill_resource_name = attrs.get("gen_ai.skill.resource.name");
    let process_executable_name = attrs.get("process.executable.name");

    if let (Some(gen_ai_tool_name), Some(gen_ai_skill_name), Some(gen_ai_skill_resource_name)) = (gen_ai_tool_name, gen_ai_skill_name, gen_ai_skill_resource_name) {
        return format!("execute_tool {gen_ai_tool_name} {gen_ai_skill_name} {gen_ai_skill_resource_name}");
    }
    if let (Some(gen_ai_tool_name), Some(gen_ai_skill_resource_name)) = (gen_ai_tool_name, gen_ai_skill_resource_name) {
        return format!("execute_tool {gen_ai_tool_name} {gen_ai_skill_resource_name}");
    }
    if let (Some(gen_ai_tool_name), Some(process_executable_name)) = (gen_ai_tool_name, process_executable_name) {
        return format!("execute_tool {gen_ai_tool_name} {process_executable_name}");
    }
    if let Some(gen_ai_tool_name) = gen_ai_tool_name {
        return format!("execute_tool {gen_ai_tool_name}");
    }

    "gen_ai.execute_tool.command.internal".to_string()
}
```

When `templates` are not specified on a span, code generation cannot infer how to construct the span name automatically. In this case, codegen makes the span name a required parameter that the caller must supply explicitly (e.g. `start_span(name: &str, ...)`).

### 4. Live-Check Validation (Planned)

In `weaver_live_check` (planned for follow-up PR), validating incoming spans against the formal specification:
1. Find the first template in `templates` where all attributes are present on the sample span (and not equal to `_OTHER` for enum attributes).
2. Format that template with the sample's attribute values -> `expected_name`.
3. If a template matched: verify `sample.name == expected_name`.
4. If mismatched, emit an advisory with actual vs expected span name.

### 5. Validation Policies

**Weaver built-in checks (implemented during loading and resolution):**
- **Syntax**: Template string must be non-empty, with matching `{` and `}`, and no nested braces.
- **Span attribute reference**: Every attribute referenced in any template must be declared or inherited on that span.

**Rego policy checks (`after_resolution` stage, planned policy proposals):**
- **Sampling relevance**: Every attribute referenced in any template must be marked `sampling_relevant: true`.
- **Attribute type**: Attributes in templates must be `string`, `int`, or an enum of strings/ints.
- **Exhaustiveness**: A span with templates must include at least one guaranteed fallback: either a static literal template (no placeholders) or a template consisting solely of required attributes that do not have `_OTHER` as an enum member.

Proposed Rego policies:

```rego
package after_resolution

import rego.v1

# 1. All attributes used in span name templates must be marked sampling_relevant: true
deny contains span_attr_violation("span_name_sampling_relevant", span.type, attr_key) if {
    some span in input.registry.spans
    some template in span.name.templates
    some attr_key in template.attributes

    some attr in span.attributes
    attr.key == attr_key
    not attr.sampling_relevant
}

# 2. Attributes used in span name templates must be string, int, or enum of strings/ints
deny contains span_attr_type_violation("span_name_invalid_attribute_type", span.type, attr_key) if {
    some span in input.registry.spans
    some template in span.name.templates
    some attr_key in template.attributes

    some attr in span.attributes
    attr.key == attr_key
    not is_allowed_template_attr_type(attr)
}

allowed_primitive_types := {"string", "int"}

is_allowed_template_attr_type(attr) if {
    is_string(attr.type)
    allowed_primitive_types[attr.type]
}

is_allowed_template_attr_type(attr) if {
    is_object(attr.type)
    some _ in attr.type.members
    every member in attr.type.members {
        is_allowed_enum_value(member.value)
    }
}

is_allowed_enum_value(val) if is_string(val)
is_allowed_enum_value(val) if {
    is_number(val)
    round(val) == val
}

# 3. Span name templates must be exhaustive
deny contains span_violation("span_name_not_exhaustive", span.type) if {
    some span in input.registry.spans
    count(span.name.templates) > 0
    not has_guaranteed_template(span)
}

has_guaranteed_template(span) if {
    some template in span.name.templates
    every attr_key in template.attributes {
        is_guaranteed_attribute(span, attr_key)
    }
}

is_guaranteed_attribute(span, attr_key) if {
    some attr in span.attributes
    attr.key == attr_key
    attr.requirement_level == "required"
    not has_other_enum_member(attr)
}

has_other_enum_member(attr) if {
    some member in attr.type.members
    is_other_member(member)
}

is_other_member(member) if member.id == "_OTHER"
is_other_member(member) if member.value == "_OTHER"

span_violation(id, span_type) := {
    "id": id,
    "level": "violation",
    "signal_type": "span",
    "signal_name": span_type,
    "message": sprintf("Span '%s' templates are not exhaustive: must contain at least one template consisting solely of required attributes that cannot resolve to '_OTHER' (or a static literal fallback).", [span_type]),
}

span_attr_violation(id, span_type, attr_key) := {
    "id": id,
    "level": "violation",
    "signal_type": "span",
    "signal_name": span_type,
    "context": {"attribute_key": attr_key},
    "message": sprintf("Span '%s' references attribute '%s' in name templates, but it is not marked 'sampling_relevant: true'.", [span_type, attr_key]),
}

span_attr_type_violation(id, span_type, attr_key) := {
    "id": id,
    "level": "violation",
    "signal_type": "span",
    "signal_name": span_type,
    "context": {"attribute_key": attr_key},
    "message": sprintf("Span '%s' references attribute '%s' in name templates with an invalid type: only string, int, or an enum of strings/ints are allowed.", [span_type, attr_key]),
}
```
