//! Checked wire scalars. None of these values grants access to a resource.
//! String-backed newtypes serialize their borrowed inner text without cloning it.
//! Deserialization and explicit owned conversions retain their existing checks.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _, ser::Error as _};
use std::fmt;

/// Largest integer that a JSON/Web client can represent without integer loss.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// A rejected scalar, without the rejected value or input context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarError {
    /// An opaque identifier was empty.
    InvalidId,
    /// A digest was not exactly 64 lowercase hexadecimal characters.
    InvalidDigest,
    /// An unsigned decimal string was noncanonical or out of range.
    InvalidU64,
    /// A timestamp was not UTC RFC 3339.
    InvalidTime,
    /// A relative path was not normalized or escaped its lexical root.
    InvalidPath,
    /// A signed decimal timestamp was not canonical.
    InvalidSignedDecimal,
    /// A numeric integer was outside its safe integer range.
    InvalidInteger,
    /// A score was not finite.
    InvalidNumber,
}

impl fmt::Display for ScalarError {
    /// Write a fixed diagnostic that contains no rejected input.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidId => "invalid opaque identifier",
            Self::InvalidDigest => "invalid SHA-256 digest",
            Self::InvalidU64 => "invalid unsigned decimal string",
            Self::InvalidTime => "invalid UTC timestamp",
            Self::InvalidPath => "invalid relative path",
            Self::InvalidSignedDecimal => "invalid signed decimal timestamp",
            Self::InvalidInteger => "invalid JSON safe integer",
            Self::InvalidNumber => "non-finite number",
        })
    }
}

impl std::error::Error for ScalarError {}

/// An opaque nonempty identifier; not a UUID, path, identity, or permission.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Id(String);

impl Id {
    /// Retain `value` unchanged, or reject an empty identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, ScalarError> {
        let value = value.into();
        if value.is_empty() {
            Err(ScalarError::InvalidId)
        } else {
            Ok(Self(value))
        }
    }
    /// Borrow the unchanged opaque identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Id {
    type Error = ScalarError;
    /// Check a decoded identifier without interpreting its contents.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<Id> for String {
    /// Recover the identifier's original string.
    fn from(value: Id) -> Self {
        value.0
    }
}

/// A lowercase, 64-character SHA-256 digest, not a source identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Digest(String);
impl Digest {
    /// Check `value` without case conversion or hashing content.
    pub fn new(value: impl Into<String>) -> Result<Self, ScalarError> {
        let value = value.into();
        if value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err(ScalarError::InvalidDigest)
        }
    }
    /// Borrow the canonical hexadecimal digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Digest {
    type Error = ScalarError;
    /// Check a decoded digest.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<Digest> for String {
    /// Recover the canonical digest text.
    fn from(value: Digest) -> Self {
        value.0
    }
}

/// A full-width unsigned integer encoded only as a canonical decimal string.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct U64(u64);
impl U64 {
    /// Construct from an already exact Rust integer; no JSON number is emitted.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    /// Return the exact numeric value for bounded local arithmetic.
    pub const fn get(self) -> u64 {
        self.0
    }
}
impl Serialize for U64 {
    /// Emit a decimal string, including for values above the Web integer limit.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}
impl<'de> Deserialize<'de> for U64 {
    /// Reject JSON numbers, leading zeroes, signs, whitespace, and overflow.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if !canonical_unsigned(&text) {
            return Err(D::Error::custom(ScalarError::InvalidU64));
        }
        text.parse::<u64>()
            .map(Self)
            .map_err(|_| D::Error::custom(ScalarError::InvalidU64))
    }
}

/// Whether a nonempty decimal string has only digits and no redundant zeroes.
fn canonical_unsigned(text: &str) -> bool {
    !text.is_empty()
        && (text == "0" || !text.starts_with('0'))
        && text.bytes().all(|b| b.is_ascii_digit())
}

/// A canonical signed nanosecond timestamp string, with no invented bit limit.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct SignedDecimal(String);
impl SignedDecimal {
    /// Check canonical decimal notation; reject plus signs and negative zero.
    pub fn new(value: impl Into<String>) -> Result<Self, ScalarError> {
        let value = value.into();
        let digits = value.strip_prefix('-').unwrap_or(&value);
        if canonical_unsigned(digits) && value != "-0" {
            Ok(Self(value))
        } else {
            Err(ScalarError::InvalidSignedDecimal)
        }
    }
    /// Borrow the unchanged canonical timestamp.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for SignedDecimal {
    type Error = ScalarError;
    /// Check a decoded signed timestamp.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<SignedDecimal> for String {
    /// Recover the canonical timestamp text.
    fn from(value: SignedDecimal) -> Self {
        value.0
    }
}

/// A UTC RFC 3339 timestamp, retained as data rather than ordering authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct Time(String);
impl Time {
    /// Validate RFC 3339 and a UTC offset without rewriting `value`.
    pub fn new(value: impl Into<String>) -> Result<Self, ScalarError> {
        let value = value.into();
        let parsed =
            time::OffsetDateTime::parse(&value, &time::format_description::well_known::Rfc3339)
                .map_err(|_| ScalarError::InvalidTime)?;
        if !parsed.offset().is_utc() {
            return Err(ScalarError::InvalidTime);
        }
        Ok(Self(value))
    }
    /// Borrow the original validated UTC timestamp.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Time {
    type Error = ScalarError;
    /// Check a decoded UTC timestamp.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<Time> for String {
    /// Recover the original timestamp text.
    fn from(value: Time) -> Self {
        value.0
    }
}

/// Check portable normalized relative syntax, without resolving a filesystem.
fn relative_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    !value.is_empty()
        && !value.contains(['\\', '\0'])
        && !(bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// A nonempty normalized relative path; not a filesystem handle or permission.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct RelativePath(String);
impl RelativePath {
    /// Reject absolute, dot, empty, traversal, backslash, and NUL syntax.
    pub fn new(value: impl Into<String>) -> Result<Self, ScalarError> {
        let value = value.into();
        if relative_path(&value) {
            Ok(Self(value))
        } else {
            Err(ScalarError::InvalidPath)
        }
    }
    /// Borrow the normalized path; no root has been admitted.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RelativePath {
    type Error = ScalarError;
    /// Check a decoded nonempty relative path.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<RelativePath> for String {
    /// Recover the normalized path string.
    fn from(value: RelativePath) -> Self {
        value.0
    }
}

/// A browse-only relative path that also permits the empty selected-root sentinel.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct RootRelativePath(String);
impl RootRelativePath {
    /// Permit an empty browse root or validate the same syntax as `RelativePath`.
    pub fn new(value: impl Into<String>) -> Result<Self, ScalarError> {
        let value = value.into();
        if value.is_empty() || relative_path(&value) {
            Ok(Self(value))
        } else {
            Err(ScalarError::InvalidPath)
        }
    }
    /// Borrow the browse path, including its possible empty root sentinel.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for RootRelativePath {
    type Error = ScalarError;
    /// Check a decoded browse path without introducing a new root.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<RootRelativePath> for String {
    /// Recover the browse path string.
    fn from(value: RootRelativePath) -> Self {
        value.0
    }
}

/// A nonnegative JSON integer no greater than `MAX_SAFE_INTEGER`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct UnsignedInteger(u64);
impl UnsignedInteger {
    /// Reject values that a JSON/Web client cannot represent exactly.
    pub const fn new(value: u64) -> Result<Self, ScalarError> {
        if value <= MAX_SAFE_INTEGER {
            Ok(Self(value))
        } else {
            Err(ScalarError::InvalidInteger)
        }
    }
    /// Return the exact integer value.
    pub const fn get(self) -> u64 {
        self.0
    }
}
impl TryFrom<u64> for UnsignedInteger {
    type Error = ScalarError;
    /// Check a decoded nonnegative integer.
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<UnsignedInteger> for u64 {
    /// Recover an exact safe integer for JSON encoding.
    fn from(value: UnsignedInteger) -> Self {
        value.0
    }
}

/// A positive safe integer for 1-based lines, pages, slides, and file ranks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct PositiveInteger(u64);
impl PositiveInteger {
    /// Reject zero and unsafe integers.
    pub const fn new(value: u64) -> Result<Self, ScalarError> {
        if value > 0 && value <= MAX_SAFE_INTEGER {
            Ok(Self(value))
        } else {
            Err(ScalarError::InvalidInteger)
        }
    }
    /// Return the positive safe integer.
    pub const fn get(self) -> u64 {
        self.0
    }
}
impl TryFrom<u64> for PositiveInteger {
    type Error = ScalarError;
    /// Check a decoded 1-based number.
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<PositiveInteger> for u64 {
    /// Recover an exact positive JSON integer.
    fn from(value: PositiveInteger) -> Self {
        value.0
    }
}

/// A signed JSON safe integer, used where the schema does not require a count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct SafeInteger(i64);
impl SafeInteger {
    /// Reject integers outside the symmetric JSON safe range.
    pub const fn new(value: i64) -> Result<Self, ScalarError> {
        if value >= -(MAX_SAFE_INTEGER as i64) && value <= MAX_SAFE_INTEGER as i64 {
            Ok(Self(value))
        } else {
            Err(ScalarError::InvalidInteger)
        }
    }
    /// Return the exact signed integer value.
    pub const fn get(self) -> i64 {
        self.0
    }
}
impl TryFrom<i64> for SafeInteger {
    type Error = ScalarError;
    /// Check a decoded signed integer.
    fn try_from(value: i64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}
impl From<SafeInteger> for i64 {
    /// Recover an exact signed JSON integer.
    fn from(value: SafeInteger) -> Self {
        value.0
    }
}

/// A finite score; no probability or cross-index calibration is implied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FiniteNumber(f64);
impl FiniteNumber {
    /// Reject NaN and either infinity before storing a score.
    pub fn new(value: f64) -> Result<Self, ScalarError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(ScalarError::InvalidNumber)
        }
    }
    /// Return the finite score.
    pub const fn get(self) -> f64 {
        self.0
    }
}
impl Serialize for FiniteNumber {
    /// Refuse non-finite values defensively rather than serialize them as null.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if !self.0.is_finite() {
            return Err(S::Error::custom(ScalarError::InvalidNumber));
        }
        serializer.serialize_f64(self.0)
    }
}
impl<'de> Deserialize<'de> for FiniteNumber {
    /// Check a decoded score; JSON itself also refuses NaN/Infinity tokens.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}
