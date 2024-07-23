// SPDX-License-Identifier: Apache-2.0

//! The representation of a semantic convention registry path/location.

use std::fmt::Display;
use std::str::FromStr;

use once_cell::sync::Lazy;
use regex::Regex;

use crate::Error;

/// Regex to parse a registry path supporting the following formats:
/// - source
/// - source@tag
/// - source[sub_folder]
/// - source@tag[sub_folder]
static REGISTRY_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?P<source>.+?)(?:@(?P<tag>.+?))?(?:\[(?P<sub_folder>.+?)])?$")
        .expect("Invalid regex")
});

/// Path to a semantic convention registry.
/// The path can be a local directory or a Git URL.
#[derive(Debug, Clone)]
pub enum RegistryPath {
    /// Local folder path pointing to a semantic convention registry.
    LocalFolder {
        /// Path to a local folder
        path: String,
    },
    /// Local archive path containing a semantic convention registry.
    LocalArchive {
        /// Path to a local archive
        path: String,
        /// Sub-folder within the archive containing the semantic convention registry
        sub_folder: Option<String>,
    },
    /// Remote archive containing a semantic convention registry.
    RemoteArchive {
        /// URL of the remote archive
        url: String,
        /// Sub-folder within the archive containing the semantic convention registry
        sub_folder: Option<String>,
    },
    /// Git repository containing a semantic convention registry.
    GitRepo {
        /// URL of the Git repository
        url: String,
        /// Tag of the Git repository (NOT YET SUPPORTED)
        tag: Option<String>,
        /// Sub-folder within the repository containing the semantic convention registry
        sub_folder: Option<String>,
    },
}

/// Implement the `FromStr` trait for `RegistryPath`, so that it can be used as
/// a command-line argument.
impl FromStr for RegistryPath {
    type Err = Error;

    /// Parse a string into a `RegistryPath`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let captures = REGISTRY_REGEX
            .captures(s)
            .ok_or(Error::InvalidRegistryPath {
                path: s.to_owned(),
                error: "Invalid registry path".to_owned(),
            })?;
        let source = captures
            .name("source")
            .ok_or(Error::InvalidRegistryPath {
                path: s.to_owned(),
                error: "Invalid registry path. No local path or URL found".to_owned(),
            })?
            .as_str();
        let tag = captures.name("tag").map(|m| m.as_str().to_owned());
        let sub_folder = captures.name("sub_folder").map(|m| m.as_str().to_owned());

        if source.starts_with("http://") || source.starts_with("https://") {
            if source.ends_with(".zip") || source.ends_with(".tar.gz") {
                Ok(Self::RemoteArchive {
                    url: source.to_owned(),
                    sub_folder,
                })
            } else {
                Ok(Self::GitRepo {
                    url: source.to_owned(),
                    tag,
                    sub_folder,
                })
            }
        } else if source.ends_with(".zip") || source.ends_with(".tar.gz") {
            Ok(Self::LocalArchive {
                path: source.to_owned(),
                sub_folder,
            })
        } else {
            Ok(Self::LocalFolder {
                path: source.to_owned(),
            })
        }
    }
}

/// Implement the `Display` trait for `RegistryPath`, so that it can be printed
/// to the console.
impl Display for RegistryPath {
    /// Format the `RegistryPath` as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryPath::LocalFolder { path } => write!(f, "{}", path),
            RegistryPath::LocalArchive { path, sub_folder } => {
                if let Some(sub_folder) = sub_folder {
                    write!(f, "{}[{}]", path, sub_folder)
                } else {
                    write!(f, "{}", path)
                }
            }
            RegistryPath::RemoteArchive { url, sub_folder } => {
                if let Some(sub_folder) = sub_folder {
                    write!(f, "{}[{}]", url, sub_folder)
                } else {
                    write!(f, "{}", url)
                }
            }
            RegistryPath::GitRepo {
                url,
                tag,
                sub_folder,
            } => {
                let mut registry_path = url.clone();
                if let Some(tag) = tag {
                    registry_path.push_str(&format!("@{}", tag));
                }
                if let Some(sub_folder) = sub_folder {
                    registry_path.push_str(&format!("[{}]", sub_folder));
                }
                write!(f, "{}", registry_path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::registry_path::RegistryPath;

    #[test]
    fn test_registry_path() {
        // Local folder
        let registry_path_str = "path/to/registry";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::LocalFolder { path } = &registry_path {
            assert_eq!(path, registry_path_str);
        } else {
            panic!("Expected LocalFolder, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Local archive (zip)
        let registry_path_str = "http://example.com/registry.zip";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::RemoteArchive { url, sub_folder } = &registry_path {
            assert_eq!(url, registry_path_str);
            assert_eq!(*sub_folder, None);
        } else {
            panic!("Expected RemoteArchive, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Local archive with sub-folder (zip)
        let registry_path_str = "http://example.com/registry.zip[model]";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::RemoteArchive { url, sub_folder } = &registry_path {
            assert_eq!(url, "http://example.com/registry.zip");
            assert_eq!(*sub_folder, Some("model".to_owned()));
        } else {
            panic!("Expected RemoteArchive, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Local archive (tar.gz)
        let registry_path_str = "http://example.com/registry.tar.gz";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::RemoteArchive { url, sub_folder } = &registry_path {
            assert_eq!(url, registry_path_str);
            assert_eq!(*sub_folder, None);
        } else {
            panic!("Expected RemoteArchive, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Local archive with sub-folder (tar.gz)
        let registry_path_str = "http://example.com/registry.tar.gz[model]";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::RemoteArchive { url, sub_folder } = &registry_path {
            assert_eq!(url, "http://example.com/registry.tar.gz");
            assert_eq!(*sub_folder, Some("model".to_owned()));
        } else {
            panic!("Expected RemoteArchive, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Git repository
        let registry_path_str = "http://example.com/registry.git";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::GitRepo {
            url,
            tag,
            sub_folder,
        } = &registry_path
        {
            assert_eq!(url, registry_path_str);
            assert_eq!(*tag, None);
            assert_eq!(*sub_folder, None);
        } else {
            panic!("Expected GitRepo, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Git repository with sub-folder
        let registry_path_str = "http://example.com/registry.git[model]";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::GitRepo {
            url,
            tag,
            sub_folder,
        } = &registry_path
        {
            assert_eq!(url, "http://example.com/registry.git");
            assert_eq!(*tag, None);
            assert_eq!(*sub_folder, Some("model".to_owned()));
        } else {
            panic!("Expected GitRepo, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Git repository with tag
        let registry_path_str = "http://example.com/registry.git@v1.0.0";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::GitRepo {
            url,
            tag,
            sub_folder,
        } = &registry_path
        {
            assert_eq!(url, "http://example.com/registry.git");
            assert_eq!(*tag, Some("v1.0.0".to_owned()));
            assert_eq!(*sub_folder, None);
        } else {
            panic!("Expected GitRepo, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);

        // Git repository with tag and sub-folder
        let registry_path_str = "http://example.com/registry.git@v1.0.0[model]";
        let registry_path: RegistryPath = registry_path_str.parse().unwrap();
        if let RegistryPath::GitRepo {
            url,
            tag,
            sub_folder,
        } = &registry_path
        {
            assert_eq!(url, "http://example.com/registry.git");
            assert_eq!(*tag, Some("v1.0.0".to_owned()));
            assert_eq!(*sub_folder, Some("model".to_owned()));
        } else {
            panic!("Expected GitRepo, got something else");
        }
        assert_eq!(registry_path.to_string(), registry_path_str);
    }
}
