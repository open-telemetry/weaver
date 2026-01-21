# Weaver Documentation Analysis and Plan

## Analysis

### Issue #1: Better documentation for Jinja templates
- `docs/codegen.md` 
  - [x] Already has a link to minijinja in line 14
  - [x] Added external Jinja tutorial links (https://ttl255.com/jinja2-tutorial-part-2-loops-and-conditionals/ and https://jinja.palletsprojects.com/en/stable/templates)
  - [x] Made resources more prominent and helpful for first-time users
  
- `crates/weaver_forge/README.md`
  - [x] Already has Jinja tutorial links in lines 29-31
  - [ ] Links could be more prominent and better explained

### Issue #2: Document built-in helper functions (e.g., kebab_case)
- `docs/codegen.md`
  - [x] Added section documenting case converter functions like `kebab_case`
  - [x] Added link to comprehensive filter reference
  - [x] Added link to source code for implementation details
  
- `crates/weaver_forge/README.md`
  - [x] Has comprehensive filter documentation in lines 478-604
  - [x] Documents all case converter functions including kebab_case (line 504)
  - [ ] Could add source code links for developers who want to see implementation

### Issue #3: Semconv schema documentation accessibility
- `schemas/semconv-syntax.md`
  - [x] Comprehensive documentation exists
  - [x] Now linked from docs/codegen.md quick links table
  - [ ] Could add link to README.md as well
  
- `schemas/semconv.schema.json`
  - [x] JSON schema exists
  - [ ] Could add link to README.md

### Issue #4: Weaver YAML schema documentation accessibility
- `docs/weaver-config.md`
  - [x] Comprehensive documentation exists (235 lines)
  - [x] Already linked in codegen.md quick links table
  - [ ] Could add more prominent link in README.md
  
- `docs/codegen.md`
  - [x] Has a quick link table to weaver-config.md (line 5)

### Issue #5: Filter parameter documentation
- `docs/codegen.md`
  - [x] Shows filter usage in example
  - [x] Added comprehensive section explaining default behavior when no filter is applied
  - [x] Added simple filtering examples
  - [x] Added explanation of ctx variable and its interaction with filters
  
- `crates/weaver_forge/README.md`
  - [x] Has JQ Filters section starting at line 121
  - [x] Has detailed filter reference starting at line 294
  - [x] Shows ctx variable usage in line 154

## Completed Tasks

1. **Enhanced `docs/codegen.md`**:
   - ✅ Added Jinja learning resources section with external tutorial links
   - ✅ Added comprehensive section on built-in helper functions with examples
   - ✅ Added detailed filter parameter explanation with default behavior and examples
   - ✅ Added semconv schema link to quick links table

## Remaining Tasks

2. **Improve `README.md`**:
   - [ ] Add direct links to semconv schema documentation (both JSON and Markdown)
   - [ ] Make weaver.yaml documentation more discoverable

3. **Enhance `crates/weaver_forge/README.md`** (optional):
   - [ ] Add source code links for filters/functions
