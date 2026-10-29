# Mutation Testing

[`cargo-mutants`](https://mutants.rs/) changes your code in small ways (for
example, it returns a default value or deletes a match arm) and runs the tests
against each change. If the tests still pass, that behaviour is not tested.
Coverage tells you which lines ran. Mutation testing tells you which lines the
tests check.

The configuration is in `.cargo/mutants.toml`. It uses nextest, builds with the
`mutants` profile (no debug info), and excludes generated and vendored code.
Results go to `mutants.out/`, which git ignores.

## Check your change before a PR

Mutate only the lines that your branch changes:

```sh
cargo install --locked cargo-mutants
git diff $(git merge-base HEAD main) -- '*.rs' > /tmp/branch.diff
cargo mutants --list --in-diff /tmp/branch.diff    # count first
cargo mutants --in-diff /tmp/branch.diff -j 2
```

- `git diff <base>` includes uncommitted edits. It does not include untracked
  files. Run `git add -N <file>` on new `.rs` files first.
- Only the tests of the mutated crate run.
- Keep `-j` at 2 to 4. Each job copies the tree and has its own target
  directory.
- Time per mutant: a few seconds in a library crate (`weaver_diff` has 20
  mutants and takes 45s). In the root `weaver` crate (`src/`), the baseline
  takes about 90s and each mutant takes about a minute. Count the mutants
  before a large run.
- In the copied tree, `build.rs` prints "Weaver UI is out of date". You can
  ignore this warning.
- If the unmutated baseline fails, fix that first. The results mean nothing
  without a passing baseline.

## Reading the result

Look at `mutants.out/missed.txt` and `mutants.out/timeout.txt`. You can ignore
`unviable.txt`, which lists mutants that do not compile.

Each missed mutant is one of these:

- **A test gap.** The mutant changes behaviour that matters, and no test
  catches it. Add a test that asserts that behaviour. For example,
  `test_compare_equal_objects` and `test_canonicalize_json_string` in
  `crates/weaver_diff/src/lib.rs` close the gaps that `cargo mutants -p
  weaver_diff` found.
- **An equivalent mutant.** The change cannot alter what the code does, for
  example a deleted match arm when a later arm does the same thing. If it
  keeps coming back, add it to `exclude_re` in `.cargo/mutants.toml`.
- **Low value.** `Display`/`Debug` output, log text, or error-message wording.
  A test is usually not necessary.

Under load, the vdir git-clone tests, `resolution_process`, and
`registry_stats::test_cli_interface` can be slow. Before you treat a timeout as
a gap, run the crate's tests alone.

## Crates whose tests run the `weaver` binary

Some integration tests in `weaver_live_check` start the CLI with
`cargo_bin("weaver")`. A run scoped to that crate does not build the root
binary, and the baseline fails with ``CARGO_BIN_EXE_weaver` is unset``.

To test a mutant, these tests need a `weaver` binary that includes the
mutated crate. Each relink takes about a minute, so a single pass over all
670 mutants takes about 3 hours. Run it in two passes instead:

- Pass 1 skips the tests that start the CLI. Most mutants are caught here in
  a few seconds each.
- Pass 2 uses `--iterate`, which re-tests only the mutants that were not
  caught in `mutants.out/`. It builds the binary and runs all of the crate's
  tests.

The result is the same as a single full pass. Extra tests can only catch
more mutants, so a mutant that pass 1 caught is also caught by the full
suite.

```sh
# 1. Unit tests only: a few seconds per mutant
cargo mutants -p weaver_live_check -j 4 --cargo-test-arg=-E \
  --cargo-test-arg='not (binary(matchers_otlp) | binary(matchers_otlp_v1) | binary(livecheck_emit))'

# 2. Re-test only the survivors, building the binary: about a minute per mutant
cargo mutants -p weaver_live_check -j 4 --iterate \
  --cargo-test-arg=--package=weaver --cargo-test-arg=-E \
  --cargo-test-arg='package(weaver_live_check)'
```

For the whole crate, pass 1 takes about 40 minutes and pass 2 takes about 30.
Use the same flags with `--in-diff` when your branch changes this crate.

## Whole crates

To get a baseline for a crate, run `cargo mutants -p <crate> -j 2`. Large
crates such as `weaver_semconv` or `weaver_forge` take hours. Start with a
small crate.
