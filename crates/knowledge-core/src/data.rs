//! Small checked data values shared by manifest inspection and backend observations.
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// A supported ordinary mount kind, without an adapter instance or access permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendKind {
    /// User-selected local directory.
    Local,
    /// Existing user-selected OS-mounted SMB/NFS directory; never share authentication.
    Nas,
    /// OS-managed cloud folder whose residency needs real provider observations.
    CloudFolder,
    /// Visible Git working copy; remote effects remain with the history owner.
    Git,
}

/// A normalized nonempty relative UTF-8 data path, never an admitted filesystem handle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct RelativePath(String);

impl RelativePath {
    /// Validate separators/components without touching a filesystem or normalizing authored text.
    ///
    /// Returns `InvalidPath` for absolute, drive-prefixed, empty, dot, traversal, NUL, or backslash paths.
    pub fn new(value: impl Into<String>) -> Result<Self, DataError> {
        let value = value.into();
        let bytes = value.as_bytes();
        if value.is_empty()
            || value.contains(['\\', '\0'])
            || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
            || value.split('/').any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(DataError::InvalidPath);
        }
        Ok(Self(value))
    }

    /// Borrow the validated spelling; no root resolution or permission check occurs.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RelativePath {
    /// Decode a string and apply the same non-normalizing path checks as construction.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// A claimed SHA-256 value, not proof that bytes were read or that a source is admitted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// Require exactly 64 lowercase hexadecimal characters; never hash, locate, or read content.
    pub fn new(value: impl Into<String>) -> Result<Self, DataError> {
        let value = value.into();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(DataError::InvalidDigest);
        }
        Ok(Self(value))
    }

    /// Borrow the checked lowercase spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ContentDigest {
    /// Decode the claimed digest without acquiring any verification or read authority.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Input-free data-shape failures; diagnostics contain no authored values or paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataError {
    /// A relative path violated its lexical contract.
    InvalidPath,
    /// A claimed digest was not lowercase SHA-256 hex.
    InvalidDigest,
    /// A byte range was empty or overflowed its half-open end.
    InvalidRange,
}

impl fmt::Display for DataError {
    /// Emit a fixed diagnostic without disclosing the rejected input.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPath => "invalid relative data path",
            Self::InvalidDigest => "invalid content digest",
            Self::InvalidRange => "invalid byte range",
        })
    }
}

impl std::error::Error for DataError {}
