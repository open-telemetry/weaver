# Developer's Guide to using Weaver

This is an in-progress guide for how to use Weaver to develop your own Semantic Convention registry or codegen.

TODO - Add more getting started guides.

## JQ - Tips and Tricks

While JQ is extremely powerful, it can be finicky to use.  For example, fighting instances where JQ decides to repate a structure vs. nest a list can take some getting used to.

Using an explorer tool like [DevToolsDaily JQ playground](https://www.devtoolsdaily.com/jq_playground/) can dramatically improve your debugging journey in crafting the `jq` expressions you use in rendering templates.

To do so we recommend following [Creating a JSON output for your registry](#creating-a-json-output-for-your-registry) and then copy-paste your registry JSON as input into the JQ playground.

## Templates - Tips and Tricks

When designing JINJA templates, you can make use the the `debug` function to output the context of the template at any point in time.  Simply add `{{ debug(ctx.some_variable) }}` do your template and you'll get a
JSON rendered view of whatever is passed into `debug` at that portion of the template.

## Policies - Tips and tricks

The OPA policy language REGO can be complicated to learn at first.  Using an explorer tool like [The Rego Playground](https://play.openpolicyagent.org) can be a dramatic aide in live debugging policy files and 
understanding the language.  

To debug `before_resolution` policies, simply take the model YAML files and convert them to JSON (online tools work here) and then copy-paste this data into the "input" of the Rego playground.

To debug `after_resolution` policies, simply follow the [Creating a JSON output for your registry](#creating-a-json-output-for-your-registry) directions and then copy your registry's JSON into the "input" of
the Rego playground.

## Creating a JSON output for your registry

Many times it's useful to have raw JSON output of your registry for debugging or tooling. To generate this
output, simply do the following:

```bash
weaver registry resolve -r <registry> -o resolved-registry.json
```
