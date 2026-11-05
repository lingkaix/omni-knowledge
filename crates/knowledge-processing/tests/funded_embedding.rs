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
        funded::{EmbeddingControl, FundedTextProvider},
    },
    funding::Funding,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    sync::atomic::Ordering,
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
    for case in ["small", "success", "cancel", "deadline"] {
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
                    .expect("cancel/deadline cannot publish a vector");
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
    }
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
}
