# Test Coverage

CI measures coverage with `cargo tarpaulin` and uploads it to Codecov (see the
coverage job in `.github/workflows/ci.yml`). Check the lines you changed
locally before you push, instead of waiting for the Codecov comment.

## What CI measures

```sh
cargo tarpaulin --workspace --output-dir coverage --out lcov --ignore-tests \
  -e xtask -e weaver -e weaver_macros -e weaver_test_support \
  --exclude-files 'crates/weaver_forge/codegen_examples/expected_codegen/*' 'crates/weaver_live_check/tests/*'
```

- `-e weaver` excludes the root crate. Code in `src/` is not measured, and the
  tests in `tests/` do not count toward coverage. Codecov never reports on
  `src/`.
- `--ignore-tests` leaves test code out of the totals.

## Check the crates you changed

```sh
cargo install cargo-tarpaulin
cargo tarpaulin -p weaver_emit -p weaver_live_check --ignore-tests \
  --skip-clean --target-dir target/tarpaulin --out Stdout
```

- Pass one `-p` for each crate you changed.
- `--target-dir target/tarpaulin` keeps the instrumented build apart from the
  normal build, so neither one rebuilds the other. `--skip-clean` reuses it
  between runs. When the build is warm, a run takes about a minute per crate.
- The report ends with `|| Uncovered Lines:` for each file. Compare them with
  your changes: `git diff -U0 main -- crates/weaver_emit/src/lib.rs`.
- The report lists every file that was compiled. Only the files in the crates
  you selected have correct numbers.

## Reading the result

- Codecov patch coverage counts every line in the diff. This includes lines
  that `cargo fmt` only rewrapped. If such a line was not covered on `main`, it
  counts as a patch miss.
- Test the behaviour that the change adds. Do not add tests only to cover lines
  that moved.
- Tarpaulin and Codecov can differ by a line or two on expressions that span
  several lines.
