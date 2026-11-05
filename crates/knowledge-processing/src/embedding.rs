//! Local multimodal requests with explicit model, package, dimension, and space identity.

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs::File;
use std::io::Read;
use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;
use url::{Host, Url};

/// Largest media input that leaves room for base64 and JSON within the provider's 64 MiB limit.
const MAX_MEDIA_BYTES: u64 = 40 * 1024 * 1024;
/// Maximum response size accepted from the local service.
const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
/// Maximum text bytes accepted for one request; callers must split longer documents.
const MAX_TEXT_BYTES: usize = 48 * 1024;

/// A complete local embedding identity; changing any identity field requires a fresh vector index.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EmbeddingConfig {
    /// Loopback HTTP base URL, without credentials, queries, or path prefixes.
    pub endpoint: String,
    /// Exact model identifier sent to the provider and checked in its response.
    pub model: String,
    /// Required number of finite values in every vector.
    pub dimension: usize,
    /// Full SHA-256 package identity checked before document content is sent.
    pub package_fingerprint: String,
    /// Provider space identifier checked in the model list and every vector response.
    pub embedding_space: String,
    /// Total request deadline in seconds, from 1 through 600.
    pub timeout_seconds: u64,
}

impl Default for EmbeddingConfig {
    /// Selects the already validated local WeMM-Embedding-2B Apple package.
    fn default() -> Self {
        Self {
            endpoint: "http://127.0.0.1:18789".into(),
            model: "wemm-embedding-2b-apple-2048".into(),
            dimension: 2048,
            package_fingerprint: "2d481d864704eaabf57a57e77544a6439dd8a524752f74ad5b8543b7f7fe3ecb"
                .into(),
            embedding_space: "wemm-2b-apple-q8-g64-2048-2d481d864704eaab".into(),
            timeout_seconds: 300,
        }
    }
}

/// Confirmed identity and execution backend of a ready local provider.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderHealth {
    /// Current execution backend, such as MLX and Core ML.
    pub backend: String,
    /// Validated package fingerprint.
    pub package_fingerprint: String,
    /// Validated model identifier.
    pub model: String,
    /// Validated vector dimension.
    pub dimension: usize,
    /// Validated embedding space.
    pub embedding_space: String,
}

/// Marks a provider request, identity, or vector failure that must stop semantic publication.
/// Local file-read, media-type, and input-size errors do not use this marker.
#[derive(Debug)]
pub struct ProviderRequestError {
    /// Preserved diagnostic chain from the failed provider boundary.
    cause: anyhow::Error,
}

impl std::fmt::Display for ProviderRequestError {
    /// Names the failed boundary; its underlying cause remains available through Error::source.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("local embedding provider request failed")
    }
}

impl std::error::Error for ProviderRequestError {
    /// Preserves the original diagnostic without converting local media errors into provider failures.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.cause.as_ref())
    }
}

/// Sends sequential requests to a loopback provider and rejects incompatible vectors.
#[derive(Clone)]
pub struct EmbeddingClient {
    /// Fixed identity for this client.
    config: EmbeddingConfig,
    /// Canonical loopback URL; localhost is converted to a literal IP address.
    endpoint: Url,
    /// HTTP transport with redirects and ambient proxy configuration disabled.
    client: Client,
}

impl EmbeddingClient {
    /// Validates configuration and builds transport without starting or contacting a service.
    /// Returns an error for remote URLs, incomplete identities, or invalid deadlines.
    pub fn new(config: &EmbeddingConfig) -> Result<Self> {
        let mut endpoint = Url::parse(&config.endpoint).context("Invalid embedding endpoint")?;
        ensure!(
            endpoint.scheme() == "http",
            "Embedding endpoint must use local HTTP"
        );
        ensure!(
            endpoint.username().is_empty() && endpoint.password().is_none(),
            "Embedding endpoint must not contain credentials"
        );
        ensure!(
            endpoint.query().is_none() && endpoint.fragment().is_none(),
            "Embedding endpoint must not contain a query or fragment"
        );
        ensure!(
            endpoint.path().is_empty() || endpoint.path() == "/",
            "Embedding endpoint must be a base URL without a path prefix"
        );
        match endpoint.host() {
            Some(Host::Ipv4(address)) => {
                ensure!(address.is_loopback(), "Embedding endpoint must be loopback")
            }
            Some(Host::Ipv6(address)) => {
                ensure!(address.is_loopback(), "Embedding endpoint must be loopback")
            }
            Some(Host::Domain("localhost")) => {
                endpoint
                    .set_ip_host(IpAddr::from([127, 0, 0, 1]))
                    .map_err(|_| anyhow::anyhow!("Cannot canonicalize localhost"))?
            }
            _ => bail!("Embedding endpoint must use a literal loopback address or localhost"),
        }
        ensure!(
            !config.model.trim().is_empty() && !config.embedding_space.trim().is_empty(),
            "Embedding model and space identity must be explicit"
        );
        ensure!(
            config.package_fingerprint.len() == 64
                && config
                    .package_fingerprint
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit()),
            "Embedding package fingerprint must be a full SHA-256 value"
        );
        ensure!(
            (1..=8192).contains(&config.dimension),
            "Embedding dimension must be from 1 through 8192"
        );
        ensure!(
            (1..=600).contains(&config.timeout_seconds),
            "Embedding timeout must be from 1 through 600 seconds"
        );
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(config.timeout_seconds))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .build()
            .context("Cannot create local embedding transport")?;
        Ok(Self {
            config: config.clone(),
            endpoint,
            client,
        })
    }

    /// Checks readiness, full package fingerprint, and model-list identity without sending user content.
    /// Returns an error if the service is unavailable, stale, incomplete, or incompatible.
    pub fn health(&self) -> Result<ProviderHealth> {
        let health = self.read_json(self.client.get(self.url("health")?).send().context(
            "Local embedding service is unavailable; start the configured WeMM provider",
        )?)?;
        ensure!(
            health["status"].as_str() == Some("ready"),
            "Local embedding service is not ready"
        );
        ensure!(
            health["package_fingerprint"].as_str()
                == Some(self.config.package_fingerprint.as_str()),
            "Embedding package fingerprint mismatch; rebuild with the intended provider"
        );
        let models = self.read_json(
            self.client
                .get(self.url("v1/models")?)
                .send()
                .context("Cannot inspect local embedding models")?,
        )?;
        let models = models["data"]
            .as_array()
            .context("Embedding model list is missing")?;
        let model = models
            .iter()
            .find(|entry| entry["id"].as_str() == Some(self.config.model.as_str()))
            .context("Configured embedding model is unavailable")?;
        ensure!(
            model["dimension"].as_u64() == Some(self.config.dimension as u64),
            "Embedding model dimension mismatch"
        );
        ensure!(
            model["embedding_space"].as_str() == Some(self.config.embedding_space.as_str()),
            "Embedding model space mismatch; existing vectors cannot be mixed"
        );
        Ok(ProviderHealth {
            backend: health["backend"].as_str().unwrap_or("unknown").into(),
            package_fingerprint: self.config.package_fingerprint.clone(),
            model: self.config.model.clone(),
            dimension: self.config.dimension,
            embedding_space: self.config.embedding_space.clone(),
        })
    }

    /// Embeds bounded UTF-8 text after a fresh provider identity check.
    /// Returns only finite, nonzero vectors with the configured model and dimension.
    pub fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        ensure!(!text.trim().is_empty(), "Embedding text must not be empty");
        ensure!(
            text.len() <= MAX_TEXT_BYTES,
            "Embedding text exceeds 48 KiB; split it into chunks"
        );
        self.embed(json!({"model": self.config.model, "input": text}))
    }

    /// Embeds a local image as a base64 data URI after checking the provider identity.
    /// Returns an error for empty, oversized, unreadable, or unsupported image files.
    pub fn embed_image(&self, path: &Path) -> Result<Vec<f32>> {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ensure!(
            mime.type_().as_str() == "image",
            "Image file has no supported media type"
        );
        self.embed_media(path, mime.as_ref(), "image_url")
    }

    /// Embeds one local video clip; callers retain its source start and end times.
    /// Use `media::segment_video` to keep clips within the model's practical limits.
    pub fn embed_video(&self, path: &Path) -> Result<Vec<f32>> {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        ensure!(
            mime.type_().as_str() == "video",
            "Video file has no supported media type"
        );
        self.embed_media(path, mime.as_ref(), "video_url")
    }

    /// Reads bounded media bytes and wraps one visual input in the provider's message format.
    fn embed_media(&self, path: &Path, mime: &str, kind: &str) -> Result<Vec<f32>> {
        let file =
            File::open(path).with_context(|| format!("Cannot read media {}", path.display()))?;
        ensure!(
            file.metadata()?.len() <= MAX_MEDIA_BYTES,
            "Embedding media exceeds 40 MiB; convert or segment the source"
        );
        let mut bytes = Vec::new();
        file.take(MAX_MEDIA_BYTES + 1).read_to_end(&mut bytes)?;
        ensure!(
            !bytes.is_empty() && bytes.len() as u64 <= MAX_MEDIA_BYTES,
            "Embedding media is empty or exceeds 40 MiB"
        );
        let source = format!("data:{mime};base64,{}", STANDARD.encode(bytes));
        self.embed(json!({"model": self.config.model, "messages": [{"role": "user", "content": [{"type": kind, kind: {"url": source}}]}]}))
    }

    /// Validates identity before content transmission and validates identity again in the result.
    fn embed(&self, payload: Value) -> Result<Vec<f32>> {
        self.request_embedding(payload)
            .map_err(|cause| anyhow::Error::new(ProviderRequestError { cause }))
    }

    /// Executes the provider boundary; callers wrap failures without changing local input failures.
    fn request_embedding(&self, payload: Value) -> Result<Vec<f32>> {
        self.health()?;
        let response = self.read_json(
            self.client
                .post(self.url("v1/embeddings")?)
                .header("X-Indexed-Timeout-MS", self.config.timeout_seconds * 1000)
                .json(&payload)
                .send()
                .context("Local embedding request failed")?,
        )?;
        ensure!(
            response["model"].as_str() == Some(self.config.model.as_str()),
            "Embedding response model mismatch"
        );
        ensure!(
            response["indexed"]["dimension"].as_u64() == Some(self.config.dimension as u64),
            "Embedding response dimension mismatch"
        );
        ensure!(
            response["indexed"]["embedding_space"].as_str()
                == Some(self.config.embedding_space.as_str()),
            "Embedding response space mismatch"
        );
        let data = response["data"]
            .as_array()
            .context("Embedding response has no data")?;
        ensure!(
            data.len() == 1 && data[0]["index"].as_u64() == Some(0),
            "Embedding response must contain exactly one vector at index zero"
        );
        let values = data[0]["embedding"]
            .as_array()
            .context("Embedding response has no vector")?;
        ensure!(
            values.len() == self.config.dimension,
            "Embedding vector has the wrong length"
        );
        let vector: Vec<f32> = values
            .iter()
            .map(|value| {
                value
                    .as_f64()
                    .map(|number| number as f32)
                    .context("Embedding vector contains a nonnumeric value")
            })
            .collect::<Result<_>>()?;
        validate_vector(&vector)?;
        Ok(vector)
    }

    /// Joins an API path to the validated base URL.
    fn url(&self, path: &str) -> Result<Url> {
        self.endpoint
            .join(path)
            .context("Cannot construct local embedding URL")
    }

    /// Reads bounded JSON without echoing response bodies that might contain private content.
    fn read_json(&self, response: Response) -> Result<Value> {
        ensure!(
            response.status().is_success(),
            "Local embedding service returned HTTP {}",
            response.status()
        );
        let mut bytes = Vec::new();
        response
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut bytes)
            .context("Cannot read local embedding response")?;
        ensure!(
            bytes.len() as u64 <= MAX_RESPONSE_BYTES,
            "Local embedding response exceeds 2 MiB"
        );
        serde_json::from_slice(&bytes).context("Local embedding response is not valid JSON")
    }
}

/// Rejects empty, nonfinite, and zero-norm vectors before they reach durable indexes.
pub fn validate_vector(vector: &[f32]) -> Result<()> {
    ensure!(!vector.is_empty(), "Embedding vector must not be empty");
    ensure!(
        vector.iter().all(|value| value.is_finite()),
        "Embedding vector contains a nonfinite value"
    );
    let norm: f64 = vector.iter().map(|value| f64::from(*value).powi(2)).sum();
    ensure!(
        norm.is_finite() && norm > 0.0,
        "Embedding vector has no finite nonzero norm"
    );
    Ok(())
}

/// Returns cosine similarity for equal-length, finite, nonzero vectors.
/// A similarity value ranks retrieval candidates; it is not a probability of correctness.
pub fn cosine_similarity(left: &[f32], right: &[f32]) -> Result<f32> {
    ensure!(
        left.len() == right.len(),
        "Cannot compare vectors with different dimensions"
    );
    validate_vector(left)?;
    validate_vector(right)?;
    let dot: f64 = left
        .iter()
        .zip(right)
        .map(|(a, b)| f64::from(*a) * f64::from(*b))
        .sum();
    let left_norm: f64 = left.iter().map(|value| f64::from(*value).powi(2)).sum();
    let right_norm: f64 = right.iter().map(|value| f64::from(*value).powi(2)).sum();
    Ok((dot / (left_norm.sqrt() * right_norm.sqrt())).clamp(-1.0, 1.0) as f32)
}

#[cfg(test)]
mod transport {
    use super::{EmbeddingClient, EmbeddingConfig};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn ephemeral_loopback_provider_returns_its_declared_vector() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_ne!(port, 18789);
        let fingerprint = "a".repeat(64);
        thread::spawn(move || {
            for _ in 0..3 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut buffer = [0; 4096];
                let size = socket.read(&mut buffer).unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..size]);
                let body = if request.contains("/health") {
                    format!(
                        r#"{{"status":"ready","package_fingerprint":"{fingerprint}","backend":"fixture"}}"#
                    )
                } else if request.contains("/v1/models") {
                    r#"{"data":[{"id":"fixture-model","dimension":2,"embedding_space":"fixture-space"}]}"#.into()
                } else {
                    r#"{"model":"fixture-model","indexed":{"dimension":2,"embedding_space":"fixture-space"},"data":[{"index":0,"embedding":[1.0,0.0]}]}"#.into()
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes());
            }
        });
        let client = EmbeddingClient::new(&EmbeddingConfig {
            endpoint: format!("http://127.0.0.1:{port}"),
            model: "fixture-model".into(),
            dimension: 2,
            package_fingerprint: "a".repeat(64),
            embedding_space: "fixture-space".into(),
            timeout_seconds: 5,
        })
        .unwrap();
        assert_eq!(client.embed_text("synthetic").unwrap(), vec![1.0, 0.0]);
    }
}
