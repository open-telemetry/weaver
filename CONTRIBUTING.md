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

### How to suggest semantic convention schema changes

Before introducing any non-trivial schema changes, we recommend discussing them in the
[semantic-conventions](https://github.com/open-telemetry/semantic-conventions) repo.

Changes affecting semantic conventions schema should be reflected in the formal schema definition and require approval from @open-telemetry/specs-semconv-approvers.

Semantic conventions schema is formally defined in [semconv.schema.json](./schemas/semconv.schema.json),
human-readable documentation is available in [semconv-syntax.md](./schemas/semconv-syntax.md).

### Creating a New Release for the Weaver Project

To create a new release for the Weaver project, follow these steps. This process ensures that the release is properly
versioned, documented, and published using our CI/CD pipeline.

#### 1. Prepare a Pull Request

Before creating a release tag, you need to prepare a Pull Request (PR) with the following updates:

1. **Update Versions**: Bump the version numbers for all crates in the project.
2. **Update CHANGELOG.md**: Add appropriate entries in `CHANGELOG.md` to reflect the new release, detailing the changes,
enhancements, and bug fixes.

##### Steps:

1. **Checkout a new branch**:
    ```bash
    git checkout -b prepare-release-vX.Y.Z
    ```

2. **Update the version numbers**: Update the `Cargo.toml` files in each crate to reflect the new version.

3. **Update CHANGELOG.md**: Add a new section for the upcoming release version and list all relevant changes.

4. **Commit your changes**:
    ```bash
    git add .
    git commit -m "Prepare release vX.Y.Z"
    ```

5. **Push your branch**:
    ```bash
    git push origin prepare-release-vX.Y.Z
    ```

6. **Open a Pull Request**: Go to the GitHub repository and open a PR from your branch. Request reviews and wait for
approval.

#### 2. Merge the PR

Once the PR is reviewed and approved, merge it into the `main` branch.

#### 3. Create and Push a Signed Tag

After merging the PR, create a signed tag for the new release.

##### Steps:

1. **Checkout the `main` branch**:
    ```bash
    git checkout main
    git pull origin main
    ```

2. **Create a signed tag**:
    ```bash
    git tag -s vX.Y.Z -m "Release vX.Y.Z"
    ```
   Replace `X.Y.Z` with the new version number. You will be prompted to enter your GPG passphrase to sign the tag.

3. **Push the tag to the upstream repository**:
    ```bash
    git push upstream vX.Y.Z
    ```

#### 4. Monitor the CI/CD Process

After pushing the tag, the GitHub Actions workflow configured in
[`.github/workflows/release.yml`](https://github.com/open-telemetry/weaver/blob/main/.github/workflows/release.yml) will
automatically detect the new tag. This workflow uses `cargo-dist` to create the release and its artifacts.

##### Steps:

1. **Check the Actions Tab**: Go to the "Actions" tab in the GitHub repository to monitor the CI/CD process. Four workflows should be triggered:
   - **CI**
   - **Release**
   - **Spelling**
   - **Weaver Docker Generator**

2. **Ensure All Workflows Complete Successfully**: Wait for all four workflows to complete. Each workflow should finish without errors.

#### 5. Verify and Update Release Description

Once all workflows are successful:

1. **Check the Release List**: Go to the "Releases" section of the Weaver project on GitHub.
2. **Update the Release Description**: Edit the release description to ensure it is complete and informative. At this
time, we do not have an automation for generating a detailed release description, so this step needs to be done manually.

#### 6. Announce the Release

The final step is to announce the new release:

1. **Post in the Slack Channel**: Inform the team and the community about the new release by posting an announcement in
the relevant Slack channel. Include a summary of the key changes and a link to the release notes.

### Repository background

The OpenTelemetry Weaver was initially developed in the
`github.com/f5/otel-weaver` repository.


## Approvers and Maintainers

For github groups see the [codeowners](CODEOWNERS) file.

### Maintainers

- [Jeremy Blythe](https://github.com/jerbly) Evertz
- [Josh Suereth](https://github.com/jsuereth) Google LLC
- [Laurent Quérel](https://github.com/lquerel) F5 Networks

For more information about the maintainer role, see the [community repository](https://github.com/open-telemetry/community/blob/main/guides/contributor/membership.md#maintainer).

### Approvers

We're seeking approvers.

For more information about the approver role, see the [community repository](https://github.com/open-telemetry/community/blob/main/guides/contributor/membership.md#approver).
