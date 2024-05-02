# Semantic Convention markdown updater

Status: **Work-In-Progress**

This crate duplicates the semconv templating from open-telemetry/build-tools.  It enables
generating "snippet" templates inside existing Markdown documents.


## Snippet Definitions

This crate can update (or diff) (`.md`) files with snippets, like so:

```markdown
# My Markdown file

<!-- semconv some.group.id -->
This content will be replaced by generated snippet.
<!-- endsemconv -->
```

Snippets can be defined with the following pseudo-grammar:

```
SNIPPET_TAG = "semconv" GROUP_ID SNIPPET_ARGS?
GROUP_ID = ('A'-'Z', 'a'-'z', '.', '_', '-')+
SNIPPET_ARGS = "(" SNIPPET_ARG ("," SNIPPET_ARG)* ")"
SNIPPET_ARG = 
   "full" |
   "metric_table" |
   "omit_requirement_level" |
   ("tag" "=" ('A'-'Z','a'-'z','0'-'9')+)
```


## Snippet Templates

You can use `weaver_forge` and `minijinja` templates for snippet generation.  When doing so, a template named
`snippet.md.j2` will be used for all snippet generation.

The template will be passed the following context variables:

- `group`: The resolved semantic convention group, referenced by id in the snippet tag.
- `snippet_type`: Either `metric_table` or `attribute_table`, based on arguments to the snippet tag.
- `tag_filter`: The set of all values defined as tag filters.
- `attribute_registry_base_url`: Base url to use when making attribute registry links.

Otherwise, the template will be given all filters, tests and functions defined in `weaver_forge`.