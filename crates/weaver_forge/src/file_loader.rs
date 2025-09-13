// SPDX-License-Identifier: Apache-2.0

//! Set of supported template loaders

use std::path::{Path, PathBuf};
use std::{fs, io};

use minijinja::ErrorKind;
use walkdir::WalkDir;

use crate::error::Error;
use crate::error::Error::TargetNotSupported;

/// An abstraction for loading files from a file system, embedded directory, etc.
pub trait FileLoader {
    /// Returns a textual representation of the root path of the loader.
    /// This representation is mostly used for debugging and logging purposes.
    fn root(&self) -> &Path;

    /// Returns a list of all files in the loader's root directory.
    fn all_files(&self) -> Vec<PathBuf>;

    /// Returns the content of a file from a given name.
    fn load_file(&self, file: &str) -> Result<Option<FileContent>, Error>;
}

/// A struct that represents the content of a file.
#[derive(Debug)]
pub struct FileContent {
    /// The path from which the file was loaded.
    pub path: PathBuf,
    /// The content of the file.
    pub content: String,
}

impl FileContent {
    /// Creates a new file content from a path or returns an error if the path is invalid.
    pub fn try_from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref().to_path_buf();
        let content = fs::read_to_string(path.clone()).map_err(|e| Error::FileLoaderError {
            file: path.clone(),
            error: e.to_string(),
        })?;
        Ok(Self { path, content })
    }
}

/// A loader that loads files from the embedded directory in the binary of Weaver or
/// from the file system if the `local_dir/target` directory exists.
///
/// This is useful for loading templates and other files that are embedded in the binary but
/// can be overridden by the user.
pub struct EmbeddedFileLoader {
    target: String,
    embedded_dir: &'static include_dir::Dir<'static>,
    fs_loader: Option<FileSystemFileLoader>,
}

impl EmbeddedFileLoader {
    /// Create a new embedded file loader.
    ///
    /// If the `local_dir/target` directory exists, the loader will use the file system loader.
    /// Otherwise, it will use the embedded directory.
    pub fn try_new(
        embedded_dir: &'static include_dir::Dir<'static>,
        local_dir: PathBuf,
        target: &str,
    ) -> Result<Self, Error> {
        let target_embedded_dir = embedded_dir.get_dir(target).ok_or_else(|| TargetNotSupported {
            root_path: embedded_dir.path().to_string_lossy().to_string(),
            target: target.to_owned(),
            error: "Target not found".to_owned(),
        })?;

        let target_local_dir = local_dir.join(target);
        let fs_loader = if target_local_dir.exists() {
            Some(FileSystemFileLoader::try_new(local_dir, target)?)
        } else {
            None
        };

        if fs_loader.is_some() {
            log::debug!("Using local templates from `{}`", target_local_dir.display());
        } else {
            log::debug!("No local templates found at `{}`. Using embedded templates.", target_embedded_dir.path().display());
        }

        Ok(Self {
            target: target.to_owned(),
            embedded_dir: target_embedded_dir,
            fs_loader,
        })
    }
}

impl FileLoader for EmbeddedFileLoader {
    /// Returns a textual representation of the root path of the loader.
    /// This representation is mostly used for debugging and logging purposes.
    fn root(&self) -> &Path {
        if let Some(fs_loader) = &self.fs_loader {
            return fs_loader.root();
        }
        self.embedded_dir.path()
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

        if let Some(fs_loader) = &self.fs_loader {
            return fs_loader.all_files();
        }

        let mut files = vec![];
        collect_files(self.embedded_dir, &mut files);
        files
    }

    /// Returns the content of a file from a given name.
    fn load_file(&self, file: &str) -> Result<Option<FileContent>, Error> {
        if let Some(fs_loader) = &self.fs_loader {
            return fs_loader.load_file(file);
        }

        let name = format!("{}/{}", self.target, file);
        match self.embedded_dir.get_file(name) {
            Some(file) => Ok(Some(FileContent {
                content: file
                    .contents_utf8()
                    .ok_or_else(|| Error::FileLoaderError {
                        file: file.path().to_owned(),
                        error: "Failed to read file contents".to_owned(),
                    })?
                    .to_owned(),
                path: file.path().to_path_buf(),
            })),
            None => Ok(None),
        }
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
    fn load_file(&self, file: &str) -> Result<Option<FileContent>, Error> {
        let path = if let Ok(path) = safe_join(&self.dir, file) {
            path
        } else {
            return Ok(None);
        };
        match fs::read_to_string(path.clone()) {
            Ok(result) => Ok(Some(FileContent {
                content: result,
                path,
            })),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(Error::FileLoaderError {
                file: path,
                error: err.to_string(),
            }),
        }
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
            format!("Failed to canonicalize root path: {e}"),
        )
    })?;
    let canonical_combined = path.canonicalize().map_err(|e| {
        let curr_dir = std::env::current_dir().expect("Failed to get current directory");
        minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!("Failed to canonicalize the path '{}' (error: {}). The current working directory is '{}'", path.display(), e, curr_dir.display()),
        )
    })?;

    // Verify that the canonical combined path starts with the canonical root path
    if canonical_combined.starts_with(&canonical_root) {
        Ok(canonical_combined)
    } else {
        Err(minijinja::Error::new(
            ErrorKind::InvalidOperation,
            format!(
                "The combined path is not a subdirectory of the root path: {canonical_root:?} -> {canonical_combined:?}"
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use include_dir::{include_dir, Dir};

    use super::*;

    static EMBEDDED_TEMPLATES: Dir<'_> = include_dir!("crates/weaver_forge/templates");

    #[test]
    fn test_template_loader() {
        let embedded_loader = EmbeddedFileLoader::try_new(
            &EMBEDDED_TEMPLATES,
            PathBuf::from("./does-not-exist"),
            "test",
        )
        .unwrap();
        let embedded_content = embedded_loader.load_file("group.md");
        assert!(embedded_content.is_ok());
        let embedded_content = embedded_content.unwrap().unwrap();
        assert!(embedded_content
            .content
            .contains("# Group `{{ ctx.id }}` ({{ ctx.type }})"));

        let overloaded_embedded_loader = EmbeddedFileLoader::try_new(
            &EMBEDDED_TEMPLATES,
            PathBuf::from("./overloaded-templates"),
            "test",
        )
        .unwrap();
        let overloaded_embedded_content = overloaded_embedded_loader.load_file("group.md");
        assert!(overloaded_embedded_content.is_ok());
        let overloaded_embedded_content = overloaded_embedded_content.unwrap().unwrap();
        assert!(overloaded_embedded_content
            .content
            .contains("# Overloaded Group `{{ ctx.id }}` ({{ ctx.type }})"));

        let fs_loader =
            FileSystemFileLoader::try_new(PathBuf::from("./templates"), "test").unwrap();
        let fs_content = fs_loader.load_file("group.md");
        assert!(fs_content.is_ok());
        let fs_content = fs_content.unwrap().unwrap();
        assert!(fs_content
            .content
            .contains("# Group `{{ ctx.id }}` ({{ ctx.type }})"));

        // Test content equality between embedded and file system loaders
        assert_eq!(embedded_content.content, fs_content.content);

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
        assert_eq!(embedded_files.len(), 18);
        let fs_files: HashSet<PathBuf> = fs_loader.all_files().into_iter().collect();
        assert_eq!(fs_files.len(), 18);
        // Test that the files are the same between the embedded and file system loaders
        assert_eq!(embedded_files, fs_files);
        // Test that all the files can be loaded from the embedded loader
        for file in embedded_files {
            let content = embedded_loader.load_file(&file.to_string_lossy()).unwrap();
            assert!(content.is_some());
        }
        // Test that all the files can be loaded from the file system loader
        for file in fs_files {
            let content = fs_loader.load_file(&file.to_string_lossy()).unwrap();
            assert!(content.is_some());
        }

        // Test case where the file does not exist
        let embedded_content = embedded_loader.load_file("missing_file.md");
        assert!(embedded_content.is_ok());
        assert!(embedded_content.unwrap().is_none());

        let fs_content = fs_loader.load_file("missing_file.md");
        assert!(fs_content.is_ok());
        assert!(fs_content.unwrap().is_none());

        // Test case where the file is outside the root directory
        // This should return no content although the file exists
        // This is because the file is not a subdirectory of the root directory
        // and is considered unsafe to load. This is a security measure to prevent
        // loading files outside the root directory.
        // It's None instead of an error because the contract of the loader is to return None
        // if the file does not exist (cf MiniJinja doc).
        let fs_content = fs_loader.load_file("../../Cargo.toml").unwrap();
        assert!(fs_content.is_none());
    }

    #[test]
    fn test_embedded_loader_error() {
        let embedded_loader = EmbeddedFileLoader::try_new(
            &EMBEDDED_TEMPLATES,
            PathBuf::from("./does-not-exist"),
            "doesn't-exist",
        );

        assert!(embedded_loader.is_err());
    }

    #[test]
    fn test_file_system_loader_error() {
        let fs_loader =
            FileSystemFileLoader::try_new(PathBuf::from("./templates"), "doesn't-exist");

        assert!(fs_loader.is_err());
    }
}
