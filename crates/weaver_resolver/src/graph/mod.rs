// SPDX-License-Identifier: Apache-2.0

//! Shared, version-neutral resolution primitives.

pub(crate) mod chosen_versions;
pub(crate) mod conflict_strategy;
pub(crate) mod entity_lookup;
pub(crate) mod exclusion;
pub mod loader;
pub(crate) mod merge;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_graph_boundary_no_v1_imports() {
        let graph_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("graph");
        for entry in fs::read_dir(graph_dir).expect("read graph dir") {
            let entry = entry.expect("valid entry");
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "rs")
                && path.file_name().is_some_and(|name| name != "mod.rs")
            {
                let content = fs::read_to_string(&path).expect("read file content");
                assert!(
                    !content.contains("crate::v1"),
                    "File {:?} violates graph architectural boundary: contains `crate::v1`",
                    path.file_name().unwrap()
                );
            }
        }
    }
}
