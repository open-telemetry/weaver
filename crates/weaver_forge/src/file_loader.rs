// SPDX-License-Identifier: Apache-2.0

//! Set of supported template loaders

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{fs, io};

use crate::error::Error;
use crate::error::Error::TargetNotSupported;
use minijinja::ErrorKind;
use walkdir::WalkDir;

/// An abstraction for loading files from a file system, embedded directory, etc.
pub trait FileLoader: Send + Sync {
    /// Returns a textual representation of the root path of the loader.
    /// This representation is mostly used for debugging and logging purposes.
    fn root(&self) -> &Path;

    /// Returns a list of all files in the loader's root directory.
    fn all_files(&self) -> Vec<PathBuf>;

    /// Returns a function that loads a file from a given name
    fn file_loader(
        &self,
    ) -> Arc<dyn for<'a> Fn(&'a str) -> Result<Option<String>, Error> + Send + Sync + 'static>;
}

/// A loader that loads files from the embedded directory in the binary of Weaver.
pub struct EmbeddedFileLoader {
    target: String,
    dir: &'static include_dir::Dir<'static>,
}

impl EmbeddedFileLoader {
    /// Create a new embedded file loader
    pub fn try_new(dir: &'static include_dir::Dir<'static>, target: &str) -> Result<Self, Error> {
        let target_dir = dir.get_dir(target);
        if let Some(dir) = target_dir {
            Ok(Self {
                target: target.to_owned(),
                dir,
            })
        } else {
            Err(TargetNotSupported {
                root_path: dir.path().to_string_lossy().to_string(),
                target: target.to_owned(),
                error: "Target not found".to_owned(),
            })
        }
    }
}

impl FileLoader for EmbeddedFileLoader {
    /// Returns a textual representation of the root path of the loader.
    /// This representation is mostly used for debugging and logging purposes.
    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Returns a list of all files in the loader's root directory.
    fn all_files(&self) -> Vec<PathBuf> {
        fn collect_files<'a>(dir: &'a include_dir::Dir<'a>, paths: &mut Vec<PathBuf>) {
            for entry in dir.entries() {
                match entry {
                    include_dir::DirEntry::Dir(d) => collect_files(d, paths),
                    include_dir::DirEntry::File(f) => {
                        let relative_path = f.path().strip_prefix(dir.path()).expect("Failed to strip prefix. Should never happen as `dir.path()` is initial root.");
                        paths.push(relative_path.to_owned());
                    }
                }
            }
        }

        let mut files = vec![];
        collect_files(self.dir, &mut files);
        files
    }

    /// Returns a function that loads a file from a given name
    fn file_loader(
        &self,
    ) -> Arc<dyn for<'a> Fn(&'a str) -> Result<Option<String>, Error> + Send + Sync + 'static> {
        let dir = self.dir;
        let target = self.target.clone();
        Arc::new(move |name| {
            let name = format!("{}/{}", target, name);
            match dir.get_file(name) {
                Some(file) => Ok(Some(
                    file.contents_utf8()
                        .ok_or_else(|| Error::FileLoaderError {
                            file: file.path().to_owned(),
                            error: "Failed to read file contents".to_owned(),
                        })?
                        .to_owned(),
                )),
                None => Ok(None),
            }
        })
    }
}

/// A loader that loads files from the file system.
pub struct FileSystemFileLoader {
    dir: PathBuf,
}

impl FileSystemFileLoader {
    /// Create a new file system loader
    pub fn try_new(dir: PathBuf, target: &str) -> Result<Self, Error> {
        let dir = safe_join(&dir, target).map_err(|e| TargetNotSupported {
            root_path: dir.to_string_lossy().to_string(),
            target: target.to_owned(),
            error: e.to_string(),
        })?;
        Ok(Self { dir })
    }
}

impl FileLoader for FileSystemFileLoader {
    /// Returns a textual representation of the root path of the loader.
    /// This representation is mostly used for debugging and logging purposes.
    fn root(&self) -> &Path {
        self.dir.as_path()
    }

    /// Returns a list of all files in the loader's root directory.
    fn all_files(&self) -> Vec<PathBuf> {
        // List all files in the target directory and its subdirectories
        let files: Vec<PathBuf> = WalkDir::new(self.dir.clone())
            .into_iter()
            .filter_map(|e| {
                // Skip directories that the owner of the running process does not
                // have permission to access
                e.ok()
            })
            .filter(|dir_entry| dir_entry.path().is_file())
            .map(|dir_entry| dir_entry.into_path()
                .strip_prefix(&self.dir)
                .expect("Failed to strip prefix. Should never happen as `self.dir` is initial root.").to_owned())
            .collect();

        files
    }

    /// Returns a function that loads a file from a given name
    /// Based on MiniJinja loader semantics, the function should return `Ok(None)` if the template
    /// does not exist.
    fn file_loader(
        &self,
    ) -> Arc<dyn for<'a> Fn(&'a str) -> Result<Option<String>, Error> + Send + Sync + 'static> {
        let dir = self.dir.clone();
        Arc::new(move |name| {
            let path = if let Ok(path) = safe_join(&dir, name) {
                path
            } else {
                return Ok(None);
            };
            match fs::read_to_string(path.clone()) {
                Ok(result) => Ok(Some(result)),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
                Err(err) => Err(Error::FileLoaderError {
                    file: path,
                    error: err.to_string(),
                }),
            }
        })
    }
}

// Combine a root path and a template name, ensuring that the combined path is
// a subdirectory of the base path.
fn safe_join(root: &Path, template: &str) -> Result<PathBuf, minijinja::Error> {
    let mut path = root.to_path_buf();
    path.push(template);

    // Canonicalize the paths to resolve any `..` or `.` components
    let canonical_root = root.canonicalize().map_err(|e| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Failed to canonicalize root path: {}", e),
        )
    })?;
    let canonical_combined = path.canonicalize().map_err(|e| {
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Failed to canonicalize combined path: {}", e),
        )
    })?;

    // Verify that the canonical combined path starts with the canonical root path
    if canonical_combined.starts_with(&canonical_root) {
        Ok(canonical_combined)
    } else {
        Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "The combined path is not a subdirectory of the root path: {:?} -> {:?}",
                canonical_root, canonical_combined
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use include_dir::{include_dir, Dir};
    use std::collections::HashSet;

    use super::*;

    static EMBEDDED_TEMPLATES: Dir<'_> = include_dir!("crates/weaver_forge/templates");

    #[test]
    fn test_template_loader() {
        let embedded_loader = EmbeddedFileLoader::try_new(&EMBEDDED_TEMPLATES, "test").unwrap();
        let embedded_content = embedded_loader.file_loader()("group.md");
        assert!(embedded_content.is_ok());
        let embedded_content = embedded_content.unwrap().unwrap();
        assert!(embedded_content.contains("# Group `{{ ctx.id }}` ({{ ctx.type }})"));

        let fs_loader =
            FileSystemFileLoader::try_new(PathBuf::from("./templates"), "test").unwrap();
        let fs_content = fs_loader.file_loader()("group.md");
        assert!(fs_content.is_ok());
        let fs_content = fs_content.unwrap().unwrap();
        assert!(fs_content.contains("# Group `{{ ctx.id }}` ({{ ctx.type }})"));

        // Test content equality between embedded and file system loaders
        assert_eq!(embedded_content, fs_content);

        // Test root path
        assert_eq!(
            embedded_loader
                .root()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            "test".to_owned()
        );
        assert_eq!(
            fs_loader
                .root()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            "test".to_owned()
        );

        // Test all files
        let embedded_files: HashSet<PathBuf> = embedded_loader.all_files().into_iter().collect();
        assert_eq!(embedded_files.len(), 17);
        let fs_files: HashSet<PathBuf> = fs_loader.all_files().into_iter().collect();
        assert_eq!(fs_files.len(), 17);
        // Test that the files are the same between the embedded and file system loaders
        assert_eq!(embedded_files, fs_files);
        // Test that all the files can be loaded from the embedded loader
        for file in embedded_files {
            let content = embedded_loader.file_loader()(&file.to_string_lossy()).unwrap();
            assert!(content.is_some());
        }
        // Test that all the files can be loaded from the file system loader
        for file in fs_files {
            let content = fs_loader.file_loader()(&file.to_string_lossy()).unwrap();
            assert!(content.is_some());
        }

        // Test case where the file does not exist
        let embedded_content = embedded_loader.file_loader()("missing_file.md");
        assert!(embedded_content.is_ok());
        assert!(embedded_content.unwrap().is_none());

        let fs_content = fs_loader.file_loader()("missing_file.md");
        assert!(fs_content.is_ok());
        assert!(fs_content.unwrap().is_none());
    }
}