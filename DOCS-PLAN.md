# Weaver Documentation Analysis and Plan

## Analysis

### Issue #1: Better documentation for Jinja templates
- `docs/codegen.md` 
  - [x] Already has a link to minijinja in line 14
  - [ ] Missing external Jinja tutorial links mentioned in the issue (https://ttl255.com/jinja2-tutorial-part-2-loops-and-conditionals/ and https://jinja.palletsprojects.com/en/stable/templates)
  - [ ] Could be more prominent and helpful for first-time users
  
- `crates/weaver_forge/README.md`
  - [x] Already has Jinja tutorial links in lines 29-31
  - [ ] Links could be more prominent and better explained

### Issue #2: Document built-in helper functions (e.g., kebab_case)
- `docs/codegen.md`
  - [ ] No mention of where case converter functions like `kebab_case` come from
  - [ ] Missing link to the source file (crates/weaver_forge/src/extensions/case.rs)
  - [ ] No comprehensive documentation of available helper functions
  
- `crates/weaver_forge/README.md`
  - [x] Has comprehensive filter documentation in lines 478-604
  - [x] Documents all case converter functions including kebab_case (line 504)
  - [ ] Could add source code links for developers who want to see implementation

### Issue #3: Semconv schema documentation accessibility
- `schemas/semconv-syntax.md`
  - [x] Comprehensive documentation exists
  - [ ] Not prominently linked from main docs or README
  - [ ] Could have a more concise summary for quick reference
  
- `schemas/semconv.schema.json`
  - [x] JSON schema exists
  - [ ] Not prominently linked from main docs or README
  
- `docs/` directory
  - [ ] Could add a dedicated page or better cross-reference to schema docs

### Issue #4: Weaver YAML schema documentation accessibility
- `docs/weaver-config.md`
  - [x] Comprehensive documentation exists (235 lines)
  - [ ] Not prominently featured in docs navigation
  - [ ] README links to it in the quick links table, but could be more prominent
  
- `docs/codegen.md`
  - [x] Has a quick link table to weaver-config.md (line 5)
  - [ ] Could add more context about what weaver.yaml is and why it's important

### Issue #5: Filter parameter documentation
- `docs/codegen.md`
  - [x] Shows filter usage in example (line 43)
  - [ ] No explanation of default behavior when no filter is applied
  - [ ] No simple filtering examples
  - [ ] No explanation of ctx variable and its interaction with filters
  
- `crates/weaver_forge/README.md`
  - [x] Has JQ Filters section starting at line 121
  - [x] Has detailed filter reference starting at line 294
  - [x] Shows ctx variable usage in line 154
  - [ ] Could add beginner-friendly explanation of default behavior
  - [ ] Could add simple examples before complex ones

## Notes

### Documentation Structure Observations
- The main documentation is split between `docs/` (user-facing) and `crates/weaver_forge/README.md` (technical reference)
- `docs/codegen.md` serves as a high-level introduction with links to detailed docs
- The Weaver Forge README is comprehensive but long (1000+ lines) - good for reference but potentially overwhelming for beginners
- Cross-linking between docs could be improved

### Key Improvements Needed
1. Add more beginner-friendly content in `docs/codegen.md`
2. Improve discoverability of existing documentation through better cross-linking
3. Add prominent links to schema documentation in README and main docs
4. Enhance filter parameter documentation with default behavior and simple examples
5. Add external Jinja learning resources more prominently

## Implementation Plan

1. **Enhance `docs/codegen.md`**:
   - Add Jinja resources section with external tutorial links
   - Add section on built-in helper functions with link to comprehensive list
   - Improve filter parameter explanation with default behavior and examples
   - Add prominent link to schema documentation

2. **Improve `README.md`**:
   - Add direct links to semconv schema documentation (both JSON and Markdown)
   - Make weaver.yaml documentation more discoverable

3. **Create or enhance schema documentation**:
   - Consider adding a quick reference guide for semconv schema
   - Ensure schema docs are easy to find from main navigation

4. **Enhance `crates/weaver_forge/README.md`** (if needed):
   - Add source code links for filters/functions
   - Improve beginner section on filters with default behavior examples
