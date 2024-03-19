# Contributing to the OpenTelemetry Weaver project

The Weaver project is a part of the [Semantic Conventions (General) SiG](https://github.com/open-telemetry/community/?tab=readme-ov-file#specification-sigs).  This group meets weekly on Mondays at 8 AM Pacific Time. The meeting is subject to change depending on contributors' availability. Check the OpenTelemetry community calendar for specific dates and for Zoom meeting links. "OTel Semconv" is the name of meeting for this group.

Meeting notes are available as a public Google doc. If you have trouble accessing the doc, please get in touch on Slack.

The meeting is open for all to join. We invite everyone to join our meeting, regardless of your experience level. Whether you're a seasoned OpenTelemetry developer, just starting your journey, or simply curious about the work we do, you're more than welcome to participate!

Additionally, Weaver has its own CNCF slack channel at [#otel-weaver](https://cloud-native.slack.com/archives/C0697EXNTL3).

## Our Development Process

### How to build  and test a change

Run `cargo xtask validate` to check the structure of the project.

Run `cargo test --all` to run the tests.

**Run `just` before any push to pre-validate all the steps performed by CI.**

### How to send Pull Request

TODO - add any special care/comments we want for clean repo.

### How to Receive Comments

- If the PR is not ready for review, please put `[WIP]` in the title or mark it as draft.
- Make sure CLA is signed and all required CI checks are clear.
- Submit small, focused PRs addressing a single concern/issue.
- Make sure the PR title reflects the contribution.
- Write a summary that helps understand the change.
- Include usage examples in the summary, where applicable.
- Include benchmarks (before/after) in the summary, for contributions that are performance enhancements.

### How to Get PRs Merged

A PR is considered to be ready to merge when:

- It has received approval from at least two Approvers. / Maintainers (of different companies).
- Major feedback is resolved.

Any Maintainer can merge the PR once it is ready to merge. Note, that some PRs may not be merged immediately if the repo is in the process of a release and the maintainers decided to defer the PR to the next release train. Also, maintainers may decide to wait for more than one approval for certain PRs, particularly ones that are affecting multiple areas, or topics that may warrant more discussion.


### Repository background

The OpenTelemetry Weaver was initially developed in the
`github.com/f5/otel-weaver` repository.


## Approvers and Maintainers

For github groups see the [codeowners](CODEOWNERS) file.

### Maintainers

- [Laurent Quérel](https://github.com/lquerel) F5 Networks
- [Josh Suereth](https://github.com/jsuereth) Google LLC

### Approvers

We're seeking approvers.

### Become an Approver or Maintainer

See the [community membership document in OpenTelemetry community repo](https://github.com/open-telemetry/community/blob/master/community-membership.md).