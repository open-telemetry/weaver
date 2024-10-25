// SPDX-License-Identifier: Apache-2.0

//! OpenTelemetry Schema Definitions
//! Please refer to the [OpenTelemetry Schema OTEP](https://github.com/open-telemetry/oteps/blob/main/text/0152-telemetry-schemas.md)
//! for more information.

use crate::Error::{InvalidTelemetrySchema, TelemetrySchemaNotFound};
use serde::{Deserialize, Serialize};
use weaver_version::Versions;

/// Errors emitted by this crate.
#[derive(thiserror::Error, Debug, Clone, Deserialize, Serialize)]
pub enum Error {
    /// OTel Telemetry schema not found.
    #[error("OTel telemetry schema not found (path_or_url: {path_or_url:?}).")]
    TelemetrySchemaNotFound {
        /// The path or the url to the telemetry schema file.
        path_or_url: String,
    },

    /// Invalid OTel Telemetry schema.
    #[error("Invalid OTel telemetry schema (path_or_url: {path_or_url:?}). {error}")]
    InvalidTelemetrySchema {
        /// The path or the url to the telemetry schema file.
        path_or_url: String,
        /// The error that occurred.
        error: String,
    },
}

/// An OpenTelemetry Telemetry Schema.
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct TelemetrySchema {
    /// Version of the file structure.
    pub file_format: String,
    /// Schema URL that this file is published at.
    pub schema_url: String,
    /// Definitions for each schema version in this family.
    /// Note: the ordering of versions is defined according to semver
    /// version number ordering rules.
    /// This section is described in more details in the OTEP 0152 and in a dedicated
    /// section below.
    /// <https://github.com/open-telemetry/oteps/blob/main/text/0152-telemetry-schemas.md>
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Versions>,
}

impl TelemetrySchema {
    /// Attempts to load a telemetry schema from a file.
    pub fn try_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Error> {
        let schema_path_buf = path.as_ref().to_path_buf();

        if !schema_path_buf.exists() {
            return Err(TelemetrySchemaNotFound {
                path_or_url: schema_path_buf.as_path().to_string_lossy().to_string(),
            });
        }

        let file = std::fs::File::open(path).map_err(|e| InvalidTelemetrySchema {
            path_or_url: schema_path_buf.as_path().to_string_lossy().to_string(),
            error: e.to_string(),
        })?;
        let reader = std::io::BufReader::new(file);
        let schema: TelemetrySchema =
            serde_yaml::from_reader(reader).map_err(|e| InvalidTelemetrySchema {
                path_or_url: schema_path_buf.as_path().to_string_lossy().to_string(),
                error: e.to_string(),
            })?;

        Ok(schema)
    }
}

#[cfg(test)]
mod tests {
    use crate::TelemetrySchema;

    #[test]
    fn test_try_from_file() {
        let schema = TelemetrySchema::try_from_file("tests/test_data/1.27.0.yaml").unwrap();
        assert_eq!(schema.file_format, "1.1.0");
        assert_eq!(schema.schema_url, "https://opentelemetry.io/schemas/1.27.0");
    }
}
