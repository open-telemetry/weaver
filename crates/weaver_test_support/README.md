# Weaver test support

Status: **Internal**

This crate provides small test-only utilities shared across weaver crates. It
is intended for use as a `[dev-dependencies]` entry only and is not published.

Currently exports:

- `reserve_test_port()` — returns a free local TCP port from a PID-derived
  lane (30000–40000), avoiding cross-binary collisions when test suites run
  in parallel.
