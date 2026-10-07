# WResult

`WResult` (`weaver_common::result::WResult`) is Weaver's 3-state outcome type
designed to support non-fatal errors (NFEs) alongside successful operations or
fatal failures.

NFEs represent issues that should be reported to the user (e.g., deprecation
notices, schema warnings, minor syntax issues, invalid examples) but do not
block Weaver from continuing subsequent processing steps.

## Variants

- `WResult::Ok(T)`: The operation completed successfully with no errors or
  warnings.
- `WResult::OkWithNFEs(T, Vec<E>)`: The operation produced a valid result `T`
  along with one or more non-fatal errors `E`.
- `WResult::FatalErr(E)`: The operation encountered an unrecoverable failure and
  could not produce `T`.

---

## Core Methods & Combinators

`WResult` provides helper methods to transform, accumulate, and bridge with
Rust's standard `Result<T, E>`:

| Method | Description |
| --- | --- |
| `WResult::with_non_fatal_errors(val, errs)` | Constructs `Ok(val)` if `errs` is empty, or `OkWithNFEs(val, errs)` if non-empty. |
| `.map(f)` | Transforms `T -> U` in `Ok` and `OkWithNFEs`, leaving `FatalErr` untouched. |
| `.extend_non_fatal_errors(vec)` | Appends additional non-fatal errors onto an existing `Ok` or `OkWithNFEs`. |
| `.capture_non_fatal_errors(&mut diag_msgs)` | Drains non-fatal errors into a `DiagnosticMessages` collector and returns standard `Result<T, E>`, enabling the `?` operator on fatal errors. |
| `.capture_warnings(&mut diag_msgs)` | Drains items with `Severity::Warning` into `DiagnosticMessages`, retaining non-warning NFEs in `WResult`. |
| `.into_result_with_non_fatal()` | Converts `WResult<T, E>` into `Result<(T, Vec<E>), E>`, allowing `?` for fatal error propagation while capturing `Vec<E>`. |
| `.into_result_failing_non_fatal()` | Converts to `Result<T, E>`, treating non-fatal errors as fatal (compounding them if needed). Commonly used in tests and strict modes. |
| `.ignore(predicate)` | Filters out non-fatal errors matching a predicate. |
| `.inspect(f)` | Observes `(&T, Option<&[E]>)` without consuming the result. |
| `.is_fatal()`, `.has_errors()`, `.num_errors()` | Checks status and error counts. |

---

## Idiomatic Patterns

### 1. Accumulating Non-Fatal Errors Across Loops

When processing collections (such as files, specs, groups, or dependencies),
iterate through items, collect successful values and non-fatal errors, and abort
early on `FatalErr`:

```rust
let mut non_fatal_errors = vec![];
let mut items = vec![];

for item in items_to_process {
    match process_item(item) {
        WResult::Ok(v) => items.push(v),
        WResult::OkWithNFEs(v, nfes) => {
            items.push(v);
            non_fatal_errors.extend(nfes);
        }
        WResult::FatalErr(e) => return WResult::FatalErr(e),
    }
}

WResult::with_non_fatal_errors(items, non_fatal_errors)
```

### 2. Validation with In-Place Error Collection

Validation methods (e.g., on specs or AST nodes) commonly accumulate non-fatal
errors in a local `Vec<Error>` and finalize with
`WResult::with_non_fatal_errors`:

```rust
impl GroupSpec {
    pub(crate) fn validate(&self, path_or_url: &str) -> WResult<(), Error> {
        let mut errors = vec![];

        if self.stability == Some(Stability::Deprecated) {
            errors.push(Error::InvalidGroupStability { ... });
        }

        // Sub-validation: propagate FatalErr or collect NFEs
        match validate_examples(...) {
            WResult::Ok(_) => {}
            WResult::OkWithNFEs(_, errs) => errors.extend(errs),
            WResult::FatalErr(err) => return WResult::FatalErr(err),
        }

        WResult::with_non_fatal_errors((), errors)
    }
}
```

### 3. Propagating Fatal Errors with `?` Using `.into_result_with_non_fatal()`

When calling a function returning `WResult` inside a function that returns
standard `Result<T, E>` or needs early return on fatal failure:

```rust
let (spec, nfes) = SemConvSpecWithProvenance::from_file(schema_url, path)
    .into_result_with_non_fatal()?;
non_fatal_errors.extend(nfes);
```

### 4. Boundary & CLI Diagnostic Collection

At pipeline or CLI entrypoints, drain non-fatal errors into a shared
`DiagnosticMessages` collector while using `?` for fatal failures:

```rust
let schema = resolver
    .resolve_repository(repo)
    .capture_non_fatal_errors(diag_msgs)?;
```

### 5. Transforming Results with `.map()` and `.extend_non_fatal_errors()`

Use `.map()` to adapt values across version abstractions or wrappers without
manually matching on all variants:

```rust
pub fn validate(self, provenance: &str) -> WResult<Self, Error> {
    match self {
        Versioned::V1(v1) => v1.validate(provenance).map(Versioned::V1),
        Versioned::V2(v2) => v2.validate(provenance).map(Versioned::V2),
    }
}
```

### 6. Testing

- **Strict assertions**: Use `.into_result_failing_non_fatal()` when a test
  expects clean success with zero warnings:

  ```rust
  let result = group.validate("<test>").into_result_failing_non_fatal();
  assert!(result.is_ok());
  ```

- **Testing warning generation**: Match on `WResult` to assert expected NFEs:

  ```rust
  match spec.validate("<test>") {
      WResult::Ok(_) => panic!("expected warning"),
      WResult::OkWithNFEs(_, nfes) => assert_eq!(nfes.len(), 1),
      WResult::FatalErr(e) => panic!("unexpected fatal error: {e:?}"),
  }
  ```

---

## Error & Diagnostic Guidelines

- Errors implement `miette::Diagnostic` and `serde::Serialize`.
- Non-fatal warnings should be annotated with
  `#[diagnostic(severity(Warning))]`.
- Errors must provide provenance context (such as file path, URL, group ID,
  or attribute name) so diagnostics are actionable.
