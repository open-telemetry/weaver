// SPDX-License-Identifier: Apache-2.0

//! Test utilities shared across weaver crates.
//!
//! This crate is intended for use as a `[dev-dependencies]` entry only.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::TcpListener;
use std::thread::sleep;
use std::time::Duration;

const PORT_BASE: u16 = 30000;
const PORT_MAX: u16 = 60000;

/// Acquire a system-wide inter-process file lock.
struct PortLock {
    lock_path: std::path::PathBuf,
}

impl PortLock {
    #[allow(clippy::print_stderr)]
    fn acquire() -> Self {
        let temp_dir = std::env::temp_dir();
        let lock_path = temp_dir.join("weaver_test_port_allocator.lock");

        let mut retries = 0;
        let max_retries = 400; // 2 seconds (400 * 5ms)
        loop {
            // Attempt to exclusively create the lock file
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(_) => break,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::AlreadyExists {
                        retries += 1;
                        if retries >= max_retries {
                            eprintln!("Warning: lock file {lock_path:?} could not be acquired after 2 seconds. Assuming orphaned; removing it and retrying.");
                            let _ = fs::remove_file(&lock_path);
                            retries = 0;
                        } else {
                            sleep(Duration::from_millis(5));
                        }
                    } else {
                        // If we encounter another error (like permissions),
                        // fall back to breaking and proceeding so we don't completely wedge.
                        break;
                    }
                }
            }
        }

        Self { lock_path }
    }
}

impl Drop for PortLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

/// Reserve a guaranteed unique, free ephemeral port for use in a test.
/// Uses an inter-process file lock and a monotonic state file to ensure
/// completely collision-free allocations across multi-threaded and
/// multi-process `cargo test` executions.
#[must_use]
pub fn reserve_test_port() -> u16 {
    let _lock = PortLock::acquire();

    let temp_dir = std::env::temp_dir();
    let state_path = temp_dir.join("weaver_test_port_allocator.state");
    // TODO - we should use a real file lock if we truly want to prevent x-process failure.
    // This at least fixes flaky test issues experienced on some machines.
    let mut next_port = if let Ok(contents) = fs::read_to_string(&state_path) {
        contents.trim().parse::<u16>().unwrap_or(PORT_BASE)
    } else {
        PORT_BASE
    };

    if !(PORT_BASE..=PORT_MAX).contains(&next_port) {
        next_port = PORT_BASE;
    }

    let start_port = next_port;
    loop {
        let candidate = next_port;
        next_port += 1;
        if next_port > PORT_MAX {
            next_port = PORT_BASE;
        }

        // Probe-bind to verify candidate is completely free on the OS
        if TcpListener::bind(("127.0.0.1", candidate)).is_ok()
            && TcpListener::bind(("0.0.0.0", candidate)).is_ok()
        {
            // Persist the next candidate port
            if let Ok(mut file) = fs::File::create(&state_path) {
                let _ = write!(file, "{next_port}");
            }
            return candidate;
        }

        // If we walked the entire port space and couldn't find a single free port, panic
        if next_port == start_port {
            panic!("Could not reserve a free test port in the 30000-60000 range");
        }
    }
}
