# Developer's Guide to using Weaver

This is an in-progress guide for how to use Weaver to develop your own Semantic Convention registry or codegen.

TODO - Add more getting started guides.

## JQ - Tips and Tricks

While JQ is extremely powerful, it can be finicky to use.  For example, fighting instances where JQ decides to repate a structure vs. nest a list can take some getting used to.

Using an explorer tool like [DevToolsDaily JQ playground](https://www.devtoolsdaily.com/jq_playground/) can dramatically improve your debugging journey in crafting the `jq` expressions you use in rendering templates.

To do so we recommend following [Creating a JSON output for your registry](#creating-a-json-output-for-your-registry) and then copy-paste your registry JSON as input into the JQ playground.

## Templates - Tips and Tricks

When designing JINJA templates, you can make use the `debug` function to output the context of the template at any point in time.  Simply add `{{ debug(ctx.some_variable) }}` to your template and you'll get a
JSON rendered view of whatever is passed into `debug` at that portion of the template.

## Policies - Tips and tricks

The OPA policy language REGO can be complicated to learn at first.  Using an explorer tool like [The Rego Playground](https://play.openpolicyagent.org) can be a dramatic aide in live debugging policy files and 
understanding the language.  

To debug `before_resolution` policies, simply take the model YAML files and convert them to JSON (online tools work here) and then copy-paste this data into the "input" of the Rego playground.

To debug `after_resolution` policies, simply follow the [Creating a JSON output for your registry](#creating-a-json-output-for-your-registry) directions and then copy your registry's JSON into the "input" of
the Rego playground.

## Creating a JSON output for your registry

Often it's useful to have raw JSON output of your registry for debugging or tooling. To generate this
output, simply do the following:

```bash
weaver registry resolve -r <registry> -o resolved-registry.json -f json
```

## Adding a new Semantic Convention Field

Let's assume that you want to add the field `display_name` to the semantic convention groups. It's an optional field
containing a string that represents the display name of the group.

1. Update the `weaver_semconv` crate to define the new field in [src/group.rs](/crates/weaver_semconv/src/group.rs). The
   data types defined in `weaver_semconv` are used to parse the registry YAML files. The following are some guidelines:
   - Optional fields should be defined as `Option<T>`. So `display_name` should be defined as `Option<String>`.
   - N-ary fields should be defined as `Vec<T>`.
   - Other data types are also supported, such as `bool`, `u64`, `f64`, `enum`, `HashMap`, etc. The only constraint is
   that the type should implement the `serde::Deserialize` and `serde::Serialize` traits.
2. Update the test data accordingly in [weaver_semconv/data](/crates/weaver_semconv/data).
3. Update all the unit tests to cover the new field. The easiest way to do this is to run the following command:
   - `cargo test --package weaver_semconv`
   - This command will run all the unit tests in the `weaver_semconv` crate.
   - The tests that fail will be the ones that need to be updated, and usually, the error message will guide you on what
   needs to be updated (most of the time, it's the addition of the new field in some struct or enum initialization).
4. Update the `weaver_resolved_schema` crate to add the corresponding new field in the "resolved" view of the registry.
This crate defines the data types used to represent the resolved view of the registry.
   - Add the new field in [weaver_resolved_schema/src/registry.rs](/crates/weaver_resolved_schema/src/registry.rs).
5. Update the `weaver_resolver` crate to define the mapping between the "non-resolved" and "resolved" views of the
registry.
   - Define the mapping in [weaver_resolver/src/registry.rs](/crates/weaver_resolver/src/registry.rs).
   - Run the unit tests to test your changes: `cargo test --package weaver_resolver`.
6. Update the `weaver_forge` crate to generate the new field in data structures used by the template engine.
   - Add the new field in [weaver_forge/src/registry.rs](/crates/weaver_forge/src/registry.rs).
   - Run the unit tests to test your changes: `cargo test --package weaver_forge`.
7. Update your templates to use the new field and use the `weaver registry generate` command to generate and test the
new templates.

Note: A simplification of the process is under development to make this process a bit easier. Indeed steps 4 and 6 are 
somewhat repetitive and could merged into a single step (see this [GH issue](https://github.com/open-telemetry/weaver/issues/208)).
