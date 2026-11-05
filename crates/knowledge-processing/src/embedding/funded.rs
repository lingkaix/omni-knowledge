//! One prepaid, affine, foreground text-provider request over the existing protocol.
//!
//! This is transport/resource composition, NOT a native source/model/receiver grant.
//! The host must retain its actual selected package, receiver intent, source/Claim and
//! policy owners. Loopback confinement and response identities never substitute for them.
use super::{EmbeddingConfig, MAX_RESPONSE_BYTES, MAX_TEXT_BYTES, ProviderRequestError};
use crate::{
    funding::{Credits, Funding},
    worker::WorkerFailure,
};
use anyhow::{Context, Result, ensure};
use knowledge_core::{admission::Holding, jobs::Reservation};
use serde_json::{Value, json};
use std::{
    future::Future,
    task::Poll,
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;
use url::Url;

/// Real host-owned current source/model/Claim checks, not an authorization assertion from wire data.
/// An implementation must retain its actual owners for the ENTIRE request/result lifetime.
pub trait EmbeddingControl {
    /// Check current selected source/model/policy and exact live Claim generation/stop request.
    fn checkpoint(&self) -> Result<()>;
    /// Actual owning job deadline; a provider timeout never enlarges it.
    fn deadline(&self) -> Instant;
}

/// One noncloneable client/runtime/resource lifetime. One request consumes it until its
/// output has been destroyed; there is no reusable output credit or second resource pool.
///
/// ```compile_fail,E0382
/// use knowledge_processing::embedding::funded::{FundedTextProvider, EmbeddingControl};
/// fn cannot_respend(p:FundedTextProvider,c:&impl EmbeddingControl){
///     let _first=p.embed_text("one",c);
///     let _second=p.embed_text("two",c);
/// }
/// ```
pub struct FundedTextProvider {
    transport: Option<Transport>,
    retained: Vec<Holding>,
}
struct Transport {
    client: Option<reqwest::Client>,
    runtime: Option<Runtime>,
    config: EmbeddingConfig,
    endpoint: Url,
}
impl Drop for Transport {
    /// No detached blocking worker exists: this current-thread runtime owns its actual HTTP tasks.
    /// Destroy the client/pool, then synchronously drop that runtime/tasks/IO BEFORE credit.
    fn drop(&mut self) {
        drop(self.client.take());
        drop(self.runtime.take());
    }
}
impl Drop for FundedTextProvider {
    fn drop(&mut self) {
        drop(self.transport.take());
        for mut holding in self.retained.drain(..) {
            let _ = holding.finish();
        }
    }
}

/// One vector and the SAME actual provider/runtime/credit owner. Vector bytes die first.
/// Copying a borrowed vector into another owner requires that consumer's separately prepaid buffer.
pub struct EmbeddedText {
    vector: Option<Vec<f32>>,
    provider: Option<FundedTextProvider>,
}
impl EmbeddedText {
    /// Borrow numeric output only after the real host checks again; this is not model-quality proof.
    pub fn vector<'a>(&'a self, control: &impl EmbeddingControl) -> Result<&'a [f32]> {
        checkpoint(control)?;
        Ok(self.vector.as_deref().expect("live affine vector"))
    }
    /// Destroy the actual output BEFORE making the SAME prepaid producer available for another call.
    pub fn into_provider(mut self) -> FundedTextProvider {
        drop(self.vector.take());
        self.provider.take().expect("live affine provider")
    }
}
impl FundedTextProvider {
    /// Complete client/runtime/2MiB response/value-parser/request/output envelope before inspection
    /// or native construction. Config facts add checked copying allowance, not spendable credit.
    pub fn requirement(config: &EmbeddingConfig) -> Result<Reservation> {
        let strings = [
            &config.endpoint,
            &config.model,
            &config.package_fingerprint,
            &config.embedding_space,
        ]
        .into_iter()
        .try_fold(0u64, |n, s| n.checked_add(s.len() as u64))
        .context("provider identity size overflow")?;
        Ok(Reservation {
            memory_bytes: strings
                .checked_mul(8)
                .and_then(|n| n.checked_add(64 * 1024 * 1024))
                .context("provider plan overflow")?,
            temporary_bytes: 0,
        })
    }
    /// Acquire the whole envelope from the SAME supplied funding BEFORE URL/client/runtime work.
    /// For `None` M3 child RSS, construct/split this sibling BEFORE running M3. Calling late does
    /// not reserve globally to repair an underfunded Claim. No network contact occurs here.
    pub fn open(config: &EmbeddingConfig, funding: &mut Funding<'_>) -> Result<Self> {
        funding.checkpoint()?;
        let need = Self::requirement(config)?;
        let mut credits = Credits::new(funding, 0, need.memory_bytes, 0, Some(0), 0)?;
        ensure!(
            tokio::runtime::Handle::try_current().is_err(),
            "funded provider requires its owning synchronous executor, not nested runtime execution"
        );
        let endpoint = super::validated_endpoint(config)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .context("cannot create owned provider runtime")?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .pool_max_idle_per_host(0)
            .build()
            .context("cannot create owned provider transport")?;
        Ok(Self {
            transport: Some(Transport {
                client: Some(client),
                runtime: Some(runtime),
                config: config.clone(),
                endpoint,
            }),
            retained: credits.retained(),
        })
    }
    /// One bounded content request. Every HTTP wait and response refill polls the host's actual
    /// currentness/stop/deadline; cancellation drops the future, then destroys its owned IO runtime.
    /// Successful output retains the whole real owner until destroyed, never a bare uncharged Vec.
    pub fn embed_text(self, text: &str, control: &impl EmbeddingControl) -> Result<EmbeddedText> {
        checkpoint(control)?;
        ensure!(!text.trim().is_empty(), "Embedding text must not be empty");
        ensure!(
            text.len() <= MAX_TEXT_BYTES,
            "Embedding text exceeds48KiB; split it into chunks"
        );
        let transport = self.transport.as_ref().expect("live affine transport");
        let outcome = transport
            .runtime
            .as_ref()
            .expect("owned runtime")
            .block_on(transport.request(text, control));
        let vector = outcome.map_err(|cause| anyhow::Error::new(ProviderRequestError { cause }))?;
        checkpoint(control)?;
        Ok(EmbeddedText {
            vector: Some(vector),
            provider: Some(self),
        })
    }
}
fn checkpoint(control: &impl EmbeddingControl) -> Result<()> {
    control.checkpoint()?;
    ensure!(Instant::now() < control.deadline(), WorkerFailure::Deadline);
    Ok(())
}
/// Poll on the same foreground thread, with no worker thread/channel left after cancellation.
async fn guarded<T>(
    operation: impl Future<Output = Result<T, reqwest::Error>>,
    control: &impl EmbeddingControl,
) -> Result<T> {
    let mut operation = std::pin::pin!(operation);
    loop {
        checkpoint(control)?;
        let mut tick = std::pin::pin!(tokio::time::sleep(Duration::from_millis(10)));
        let ready = std::future::poll_fn(|cx| {
            if let Poll::Ready(value) = operation.as_mut().poll(cx) {
                return Poll::Ready(Some(value));
            }
            match tick.as_mut().poll(cx) {
                Poll::Ready(()) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            }
        })
        .await;
        if let Some(value) = ready {
            checkpoint(control)?;
            return value.context("owned provider HTTP IO failed");
        }
    }
}
impl Transport {
    fn timeout(&self, control: &impl EmbeddingControl) -> Result<Duration> {
        checkpoint(control)?;
        Ok(control
            .deadline()
            .saturating_duration_since(Instant::now())
            .min(Duration::from_secs(self.config.timeout_seconds)))
    }
    fn url(&self, path: &str) -> Result<Url> {
        self.endpoint
            .join(path)
            .context("cannot construct provider API URL")
    }
    async fn get(&self, path: &str, control: &impl EmbeddingControl) -> Result<Value> {
        let response = guarded(
            self.client
                .as_ref()
                .expect("owned client")
                .get(self.url(path)?)
                .timeout(self.timeout(control)?)
                .send(),
            control,
        )
        .await?;
        self.read_json(response, control).await
    }
    async fn read_json(
        &self,
        mut response: reqwest::Response,
        control: &impl EmbeddingControl,
    ) -> Result<Value> {
        ensure!(
            response.status().is_success(),
            "Local embedding service returned HTTP{}",
            response.status()
        );
        let mut bytes = Vec::new();
        loop {
            checkpoint(control)?;
            let Some(chunk) = guarded(response.chunk(), control).await? else {
                break;
            };
            ensure!(
                chunk.len() <= MAX_RESPONSE_BYTES as usize - bytes.len(),
                "Local embedding response exceeds2MiB"
            );
            // The full bounded response/parser/output peak was acquired before ANY network work.
            bytes.extend_from_slice(&chunk);
        }
        checkpoint(control)?;
        let value =
            serde_json::from_slice(&bytes).context("Local embedding response is not valid JSON")?;
        checkpoint(control)?;
        Ok(value)
    }
    async fn request(&self, text: &str, control: &impl EmbeddingControl) -> Result<Vec<f32>> {
        let health = self.get("health", control).await?;
        super::check_health(&self.config, &health)?;
        let models = self.get("v1/models", control).await?;
        super::checked_models(&self.config, &health, &models)?;
        checkpoint(control)?;
        let payload = json!({"model":self.config.model,"input":text});
        let response = guarded(
            self.client
                .as_ref()
                .expect("owned client")
                .post(self.url("v1/embeddings")?)
                .header(
                    "X-Indexed-Timeout-MS",
                    self.timeout(control)?.as_millis() as u64,
                )
                .timeout(self.timeout(control)?)
                .json(&payload)
                .send(),
            control,
        )
        .await?;
        let response = self.read_json(response, control).await?;
        let vector = super::checked_embedding(&self.config, &response)?;
        checkpoint(control)?;
        Ok(vector)
    }
}
