# Changelog

All notable changes to this project will be documented in this file.

## [unreleased]

### 🚀 Features

- *(registry)* Improve resolved schema and registry api usability.
- *(registry)* Introduce the concept of named registries
- *(stats)* Implement registry stats command
- *(resolve)* Implement registry resolve command
- *(template)* Add a more complex example generating markdown files per group prefix
- *(template)* Reimplement template generation based on minijinja + jaq (jq-like filters)
- *(cli)* Add quiet mode
- *(generator)* Add support for all group types
- *(generator)* Add jq-like filter support to make artifact generation more flexible
- *(generator)* Complete the weaver registry generate command.
- *(cli)* Add update-markdown sub-command and align sub-command args in the registry command.
- *(registry)* Improve unit test to check the generated markdown
- *(registry)* Add unit test to check the generated markdown
- *(registry)* Generate markdown from jinja2 templates
- *(template)* Generate markdown files describing a registry
- *(template)* Add template syntax configuration
- *(template)* Initialize template engine with a root directory to support include clause.
- *(template)* Expose template.set_file_name method to dynamically define the file name of the output.
- *(template)* Generate registry from templates
- *(resolve)* Improve error reporting
- *(resolve)* Fix typo
- *(resolve)* Implement `include` constraint
- *(resolve)* Check `any_of` constraints
- *(template)* Integrate with minininja
- *(template)* Start integration of the case converter
- *(template)* Replace tera with minijinja to improve error handling
- *(registry)* Refactor registry sub-commands.
- *(registry)* Add `weaver check registry` command
- *(resolver)* Simplify semantic convention registry resolution function

### 🐛 Bug Fixes

- *(resolution)* Adjust other unit tests to take into account the fix
- *(resolution)* Make resolution process easy to test in unit tests
- *(resolution)* Fix resolution order
- *(resolution)* Create minimal example reproducing the bug

### 📚 Documentation

- *(template)* Add documentation to describe the template engine.
- Describe crates layout and add README.md files for every crates in the workspace.
- Clean up README.md

### 🧪 Testing

- *(integration)* Create integration test to check parsing and resolution of the official semconv repo.

### ⚙️ Miscellaneous Tasks

- *(coverage)* Improve test coverage
- *(coverage)* Remove xtask and main command line from the code coverage
- *(coverage)* Apply `tarpaulin` coverage to the entire workspace
- *(install)* Add `cargo tarpaulin` in the list of tools to install
- *(build)* Trigger ci.yml workflow for all push and pull request
- *(coverage)* Add test code coverage with cargo tarpaulin
- *(clippy)* Add more clippy lints
- *(clippy)* Fix more clippy issues
- *(clippy)* Fix explicit_into_iter_loop clippy issue
- *(git)* Make the output dir invisible for git
- *(changelog)* Add git cliff configuration
- *(code)* Make error enums non-exhaustive
- *(code)* Implement #54
- *(code)* Fix str_to_string clippy lint issues
- *(code)* Implement #54 + new clippy lint rule
- *(build)* Fix doc lint issue
- *(build)* Fix GH action
- *(build)* Add xtask
- *(build)* Replace script/check_workspace with cargo xtask validate
- *(build)* Define lint rules globally from the cargo workspace
- *(build)* Clippy lint rules to remove unwrap and enforce must_use when needed
- *(build)* Fix clippy issues
- *(doc)* Update README.md to describe check and generate sub-commands
- *(build)* Fix clippy issue
- *(build)* Fix merge issue.
- *(build)* Update cargo lock
- *(compatibility)* Align attribute type and examples definitions
- *(compatibility)* Align requirement level definition
- *(compatibility)* Align stability definition
- *(compatibility)* Make resolved registry compatible with official registry syntax
- *(clippy)* Fix clippy lint issues
- *(error)* Improve compound error management
- *(ci)* Fix toolchain version issue
- *(ci)* Attempt to fix toolchain version issue
- *(build)* Fix ci workflow
- *(build)* Fix scripts path
- *(build)* Remove allowed-external-types.toml files from the Typos control.
- *(build)* Add control procedures for workspace and public API policies
- *(build)* Run build and test only with ubuntu target for now.
- *(build)* Remove macos target for the build (API rate limit reached, we need to figure out that later).
- Add cargo lock file.
- *(dep)* Bump dependency versions
- Migrate f5/otel-weaver repo to open-telemetry/weaver repo