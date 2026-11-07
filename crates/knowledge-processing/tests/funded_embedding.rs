//! Real HTTP/Claim/resource lifecycle controls; deterministic numbers prove NO model quality/native grant.
use knowledge_core::{
    backend::Budget,
    data::ContentDigest,
    jobs::{Claim, Key, Outcome, Priority, Reservation, Submission},
};
use knowledge_processing::{
    WorkerFailure, admission,
    embedding::{
        EmbeddingConfig,
        funded::{EmbeddingControl, FundedTextProvider, ProviderRefusal},
    },
    funding::Funding,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
fn digest(b: &[u8]) -> ContentDigest {
    ContentDigest::new(format!("{:x}", Sha256::digest(b))).unwrap()
}
struct Control<'a> {
    claim: &'a Claim,
    key: Key<'a>,
    source: &'a File,
    length: u64,
    paid_error: Option<Arc<AtomicU64>>,
}
struct PaidErrorBytes {
    bytes: Vec<u8>,
    observed: Arc<AtomicU64>,
}
impl std::fmt::Debug for PaidErrorBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("actual paid error bytes")
    }
}
impl std::fmt::Display for PaidErrorBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("actual paid error bytes")
    }
}
impl std::error::Error for PaidErrorBytes {}
impl Drop for PaidErrorBytes {
    fn drop(&mut self) {
        if self.bytes.len() == 8192 {
            self.observed.store(
                admission::current()
                    .unwrap()
                    .status()
                    .unwrap()
                    .memory_available,
                Ordering::SeqCst,
            );
        }
    }
}
impl EmbeddingControl for Control<'_> {
    fn checkpoint(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.source.metadata()?.len() == self.length,
            "actual fixture source drift"
        );
        anyhow::ensure!(
            self.claim.stop_requested(self.key)?.is_none(),
            WorkerFailure::Cancelled
        );
        if let Some(observed) = &self.paid_error {
            return Err(anyhow::Error::new(PaidErrorBytes {
                bytes: vec![0x17; 8192],
                observed: observed.clone(),
            }));
        }
        Ok(())
    }
    fn deadline(&self) -> Instant {
        self.claim.deadline()
    }
}
#[path = "support/embedding_server.rs"]
mod server;
use server::{Cancel, Server};

#[test]
fn actual_claim_funded_http_is_affine_retained_and_cancellable_without_detached_work() {
    admission::establish_process_account().unwrap();
    let account = admission::current().unwrap();
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("original.txt");
    let text = "actual 日本語";
    fs::write(&path, text).unwrap();
    let source = File::open(&path).unwrap();
    let version = digest(text.as_bytes());
    let processor = digest(include_bytes!("../src/embedding/funded.rs"));
    for case in ["small", "success", "cancel", "deadline", "error-tail"] {
        let dropped_error_available = Arc::new(AtomicU64::new(u64::MAX));
        let recipe = digest(case.as_bytes());
        let key = Key {
            context: "owned-provider-fixture",
            library: "provider",
            mount: None,
            source: "original.txt",
            version: &version,
            recipe: &recipe,
            processor: &processor,
        };
        // Config/requirement facts before any network or native client construction.
        let mut config = EmbeddingConfig {
            endpoint: "http://127.0.0.1:1".into(),
            model: "protocol-fixture".into(),
            dimension: 2,
            package_fingerprint: "a".repeat(64),
            embedding_space: "contract-only-v1".into(),
            timeout_seconds: 5,
        };
        let memory = FundedTextProvider::requirement(&config)
            .unwrap()
            .memory_bytes
            + 1024;
        account
            .submit(Submission {
                operation_id: case,
                job_id: case,
                key,
                budget: Budget {
                    max_files: 1,
                    max_bytes: 4096,
                    max_memory_bytes: memory,
                    max_millis: if case == "deadline" { 150 } else { 5000 },
                },
                reservation: Reservation {
                    memory_bytes: if case == "small" { 1 } else { memory },
                    temporary_bytes: 0,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let mut claim = account.queue().claim(Instant::now()).unwrap().unwrap();
        let cancel = (case == "cancel").then(|| Cancel {
            queue: account.queue().clone(),
            job: claim.job(),
            version: version.clone(),
            recipe: recipe.clone(),
            processor: processor.clone(),
        });
        let server = Server::start(&config, cancel, matches!(case, "cancel" | "deadline"));
        config.endpoint = server.endpoint.clone();
        let baseline = account.status().unwrap().reserved;
        let mut rest = match account
            .reserve(
                Reservation {
                    memory_bytes: account.status().unwrap().memory_available,
                    temporary_bytes: 0,
                },
                knowledge_core::admission::Origin::BeforeVerification,
            )
            .unwrap()
        {
            knowledge_core::admission::Decision::Ready(h) => h,
            _ => panic!("pressure control"),
        };
        assert_eq!(account.status().unwrap().memory_available, 0);
        let provider = FundedTextProvider::open(
            &config,
            &mut Funding::Claim {
                claim: &mut claim,
                expected: key,
            },
        );
        let mut refused_available = None;
        if case == "small" {
            assert_eq!(
                provider
                    .err()
                    .unwrap()
                    .downcast_ref::<admission::AdmitError>(),
                Some(&admission::AdmitError::InsufficientCredit)
            );
            assert_eq!(
                server.accepted.load(Ordering::SeqCst),
                0,
                "admission must refuse BEFORE contact"
            );
        } else {
            let provider = provider.unwrap();
            let control = Control {
                claim: &claim,
                key,
                source: &source,
                length: text.len() as u64,
                paid_error: (case == "error-tail").then(|| dropped_error_available.clone()),
            };
            let began = Instant::now();
            let output = provider.embed_text(text, &control);
            if case == "success" {
                let result = output.unwrap();
                assert_eq!(result.vector(&control).unwrap(), &[text.len() as f32, 1.0]);
                assert_eq!(
                    account.status().unwrap().memory_available,
                    0,
                    "the actual result still owns its prepaid envelope"
                );
                let provider = result.into_provider();
                let result = provider.embed_text("second", &control).unwrap();
                assert_eq!(result.vector(&control).unwrap(), &[6.0, 1.0]);
                drop(result);
                assert_eq!(server.accepted.load(Ordering::SeqCst), 6);
                assert_eq!(server.seen.lock().unwrap().as_deref(), Some("second"));
            } else {
                let error = output
                    .err()
                    .expect("cancel/deadline/error cannot publish a vector");
                refused_available = Some(account.status().unwrap().memory_available);
                if case == "cancel" {
                    assert!(
                        error.chain().any(|e| e.downcast_ref::<WorkerFailure>()
                            == Some(&WorkerFailure::Cancelled)),
                        "{error:#}"
                    );
                }
                assert!(
                    began.elapsed() < Duration::from_millis(800),
                    "actual cooperative stop waited for slow remote completion"
                );
            }
        }
        rest.finish().unwrap();
        assert_eq!(
            account.status().unwrap().reserved.memory_bytes,
            baseline.memory_bytes
                - if case == "small" {
                    0
                } else {
                    FundedTextProvider::requirement(&config)
                        .unwrap()
                        .memory_bytes
                },
            "native runtime/output actually destroyed before credit release"
        );
        account
            .queue()
            .settle(
                &claim,
                key,
                if case == "success" {
                    Outcome::Completed
                } else if case == "cancel" {
                    Outcome::Cancelled
                } else {
                    Outcome::Failed
                },
            )
            .unwrap();
        drop(server);
        if case == "error-tail" {
            assert_eq!(
                dropped_error_available.load(Ordering::SeqCst),
                0,
                "actual paid error bytes must die BEFORE their provider credit releases"
            );
        }
        if let Some(available) = refused_available {
            assert_eq!(
                available, 0,
                "actual failed HTTP result must retain its provider/IO credit until its error owner dies"
            );
        }
    }
    continuing_claim(&source, text, &version, &processor);
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
}

fn continuing_claim(source: &File, text: &str, version: &ContentDigest, processor: &ContentDigest) {
    let account = admission::current().unwrap();
    for case in [
        "continuation-open",
        "continuation-small",
        "continuation-request",
        "continuation-success",
    ] {
        let recipe = digest(case.as_bytes());
        let key = Key {
            context: "actual-continuing-provider-fixture",
            library: "provider",
            mount: None,
            source: "original.txt",
            version,
            recipe: &recipe,
            processor,
        };
        let mut config = EmbeddingConfig {
            endpoint: "http://127.0.0.1:1".into(),
            model: "protocol-fixture".into(),
            dimension: 2,
            package_fingerprint: "a".repeat(64),
            embedding_space: "contract-only-v1".into(),
            timeout_seconds: 5,
        };
        let server = Server::start(&config, None, false);
        config.endpoint = if case == "continuation-open" {
            "http://192.0.2.1:1".into()
        } else {
            server.endpoint.clone()
        };
        let need = FundedTextProvider::requirement(&config).unwrap();
        let memory = need.memory_bytes + 65536;
        let baseline = account.status().unwrap().reserved;
        account
            .submit(Submission {
                operation_id: case,
                job_id: case,
                key,
                budget: Budget {
                    max_files: 1,
                    max_bytes: 4096,
                    max_memory_bytes: memory,
                    max_millis: 5000,
                },
                reservation: Reservation {
                    memory_bytes: memory,
                    temporary_bytes: 0,
                },
                priority: Priority::Normal,
                max_attempts: 1,
            })
            .unwrap();
        let claim = Arc::new(account.queue().claim(Instant::now()).unwrap().unwrap());
        let peer = claim.clone();
        assert!(Arc::ptr_eq(&peer, &claim));
        let mut pressure = match account
            .reserve(
                Reservation {
                    memory_bytes: account.status().unwrap().memory_available,
                    temporary_bytes: 0,
                },
                knowledge_core::admission::Origin::BeforeVerification,
            )
            .unwrap()
        {
            knowledge_core::admission::Decision::Ready(holding) => holding,
            _ => panic!("actual pressure must fit"),
        };
        let paid_memory = if case == "continuation-small" {
            1
        } else {
            need.memory_bytes
        };
        let paid = account
            .split_claim(
                &claim,
                key,
                Reservation {
                    memory_bytes: paid_memory,
                    temporary_bytes: 0,
                },
            )
            .unwrap();
        let opened = FundedTextProvider::open_prepaid(&config, vec![paid]);
        let provider = if matches!(case, "continuation-open" | "continuation-small") {
            let mut error = opened
                .err()
                .expect("nonloopback receiver must refuse without contact");
            assert_eq!(account.status().unwrap().memory_available, 0);
            let owner = error
                .downcast_mut::<ProviderRefusal>()
                .expect("actual owning refusal");
            let provider = owner
                .take_provider()
                .expect("actual failed construction credit");
            assert!(owner.take_provider().is_none(), "no duplicate loan");
            drop(error);
            provider
        } else {
            let provider = opened.unwrap();
            let control = Control {
                claim: &claim,
                key,
                source,
                length: text.len() as u64,
                paid_error: None,
            };
            let result = provider.embed_text(
                if case == "continuation-request" {
                    ""
                } else {
                    text
                },
                &control,
            );
            if case == "continuation-request" {
                let mut error = result.err().expect("empty content refuses before HTTP");
                assert_eq!(account.status().unwrap().memory_available, 0);
                let owner = error
                    .downcast_mut::<ProviderRefusal>()
                    .expect("actual request owner");
                let provider = owner.take_provider().expect("actual live client/runtime");
                assert!(owner.take_provider().is_none());
                drop(error);
                provider
            } else {
                let result = result.unwrap();
                assert_eq!(result.vector(&control).unwrap(), &[text.len() as f32, 1.0]);
                assert_eq!(account.status().unwrap().memory_available, 0);
                result.into_provider()
            }
        };
        // A live transport cannot be declared settled inside an ambient runtime. It returns
        // the actual owner; no vector/runtime/credit is replaced by a marker or fresh reservation.
        let provider = if !matches!(case, "continuation-open" | "continuation-small") {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let provider = runtime.block_on(async move {
                let provider = provider
                    .into_holdings()
                    .err()
                    .expect("return actual live owner");
                assert_eq!(account.status().unwrap().memory_available, 0);
                provider
            });
            drop(runtime);
            *provider
        } else {
            provider
        };
        let holdings = provider
            .into_holdings()
            .ok()
            .expect("actual synchronous transport settlement");
        assert_eq!(
            holdings
                .iter()
                .map(|h| h.remaining().unwrap().memory_bytes)
                .sum::<u64>(),
            paid_memory
        );
        assert_eq!(account.status().unwrap().memory_available, 0);
        for mut holding in holdings {
            account.rejoin_claim(&mut holding, &claim, key).unwrap();
            assert_eq!(
                account.status().unwrap().memory_available,
                0,
                "no global release/reserve gap"
            );
        }
        assert_eq!(
            account
                .job_status(claim.job(), key)
                .unwrap()
                .execution
                .memory_bytes,
            memory
        );
        // The returned credit is actually usable by this same continuing Claim under pressure,
        // not just a scalar status. No new model request/retry or global reserve is performed.
        let mut next = account
            .split_claim(
                &claim,
                key,
                Reservation {
                    memory_bytes: 2 * 1024 * 1024,
                    temporary_bytes: 0,
                },
            )
            .unwrap();
        account.rejoin_claim(&mut next, &claim, key).unwrap();
        assert_eq!(account.status().unwrap().memory_available, 0);
        account
            .queue()
            .settle(
                &claim,
                key,
                if case == "continuation-success" {
                    Outcome::Completed
                } else {
                    Outcome::Failed
                },
            )
            .unwrap();
        pressure.finish().unwrap();
        assert_eq!(account.status().unwrap().reserved, baseline);
        assert_eq!(
            server.accepted.load(Ordering::SeqCst),
            if case == "continuation-success" { 3 } else { 0 }
        );
        drop(server);
        eprintln!("actual provider same-Claim continuation PASS: {case}");
    }
}
