// SPDX-License-Identifier: Apache-2.0

//! Test utilities shared across weaver crates.
//!
//! This crate is intended for use as a `[dev-dependencies]` entry only.

use std::net::TcpListener;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::OnceLock;

const LANE_BASE: u16 = 30000;
const LANE_SIZE: u16 = 50;
const NUM_LANES: u16 = 200;

static NEXT_PORT: OnceLock<AtomicU16> = OnceLock::new();

fn next_port_counter() -> &'static AtomicU16 {
    NEXT_PORT.get_or_init(|| {
        let lane_idx = u16::try_from(std::process::id() % u32::from(NUM_LANES))
            .expect("lane index is bounded by NUM_LANES, which fits in u16");
        AtomicU16::new(LANE_BASE + lane_idx * LANE_SIZE)
    })
}

/// Reserve a free port for use in a test. Each test binary picks a 50-port
/// lane from a PID-derived offset (30000–40000); within the binary an
/// atomic counter walks through the lane and the candidate is verified
/// free with a probe-bind. Cross-binary collisions are vanishingly rare
/// because lanes don't overlap.
#[must_use]
pub fn reserve_test_port() -> u16 {
    let counter = next_port_counter();
    for _ in 0..(LANE_SIZE as usize * 4) {
        let port = counter.fetch_add(1, Ordering::SeqCst);
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    panic!("could not reserve a free port in this test binary's port lane")
}
