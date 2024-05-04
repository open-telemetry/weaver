// SPDX-License-Identifier: Apache-2.0

//!

use std::env;

fn main() {
    env::vars().for_each(|(key, value)| {
        println!("cargo:warning={}: {}", key, value);
    });
    env::args().for_each(|arg| {
        println!("cargo:warning=ARGS->{}", arg);
    });
    env::args_os().for_each(|arg| {
        println!("cargo:warning=AGS_OS->{:?}", arg);
    });
    _ = env::current_dir().ok().map(|path| {
        println!("cargo:warning=Current dir: {:?}", path);
    });
    _ = env::current_exe().ok().map(|path| {
        println!("cargo:warning=EXE->{}", path.display());
    });

    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:warning={}", out_dir);
}