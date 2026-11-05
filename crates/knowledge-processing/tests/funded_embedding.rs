//! Real HTTP/Claim/resource lifecycle controls; deterministic numbers prove NO model quality/native grant.
use knowledge_core::{
    backend::Budget,
    data::ContentDigest,
    jobs::{Claim, Key, Outcome, Priority, Queue, Reservation, Submission},
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
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
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
struct Cancel {
    queue: Queue,
    job: u64,
    version: ContentDigest,
    recipe: ContentDigest,
    processor: ContentDigest,
}
impl Cancel {
    fn cancel(&self) {
        self.queue
            .cancel(
                self.job,
                Key {
                    context: "owned-provider-fixture",
                    library: "provider",
                    mount: None,
                    source: "original.txt",
                    version: &self.version,
                    recipe: &self.recipe,
                    processor: &self.processor,
                },
            )
            .unwrap();
    }
}
struct Server {
    endpoint: String,
    stop: Arc<AtomicBool>,
    accepted: Arc<AtomicUsize>,
    thread: Option<JoinHandle<()>>,
    credit: Option<knowledge_core::admission::Holding>,
}
impl Server {
    fn start(cancel: Option<Cancel>, delay: bool) -> Self {
        let account = admission::current().unwrap();
        let credit = match account
            .reserve(
                Reservation {
                    memory_bytes: 65536,
                    temporary_bytes: 0,
                },
                knowledge_core::admission::Origin::BeforeVerification,
            )
            .unwrap()
        {
            knowledge_core::admission::Decision::Ready(h) => h,
            _ => panic!("fixture server admission"),
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let accepted = Arc::new(AtomicUsize::new(0));
        let count = accepted.clone();
        let thread = thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(8);
            while !stopped.load(Ordering::Acquire) && Instant::now() < end {
                let (mut socket, _) = match listener.accept() {
                    Ok(x) => x,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("fixture accept {e}"),
                };
                count.fetch_add(1, Ordering::SeqCst);
                socket
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_millis(100)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                let mut header_end = None;
                let mut length = 0usize;
                while request.len() < 65536
                    && !stopped.load(Ordering::Acquire)
                    && Instant::now() < end
                {
                    match socket.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => request.extend_from_slice(&buffer[..n]),
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                            ) =>
                        {
                            continue;
                        }
                        Err(_) => break,
                    }
                    if header_end.is_none()
                        && let Some(i) = request.windows(4).position(|w| w == b"\r\n\r\n")
                    {
                        header_end = Some(i + 4);
                        let headers = String::from_utf8_lossy(&request[..i]);
                        length = headers
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|s| s.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                    }
                    if header_end.is_some_and(|n| request.len() >= n + length) {
                        break;
                    }
                }
                let Some(header_end) = header_end else {
                    continue;
                };
                let first = String::from_utf8_lossy(&request[..header_end]);
                let body=if first.starts_with("GET /health "){serde_json::json!({"status":"ready","package_fingerprint":"a".repeat(64),"backend":"deterministic-contract-only"})}
    else if first.starts_with("GET /v1/models "){serde_json::json!({"data":[{"id":"protocol-fixture","dimension":2,"embedding_space":"contract-only-v1"}]})}
    else {
     if let Some(cancel)=&cancel{cancel.cancel();}
     if delay {let until=Instant::now()+Duration::from_secs(1);while Instant::now()<until&&!stopped.load(Ordering::Acquire){thread::sleep(Duration::from_millis(2));}}
     let supplied:serde_json::Value=serde_json::from_slice(&request[header_end..]).unwrap();let n=supplied["input"].as_str().unwrap().len();
     serde_json::json!({"model":"protocol-fixture","indexed":{"dimension":2,"embedding_space":"contract-only-v1"},"data":[{"index":0,"embedding":[n as f64,1.0]}]})
    }.to_string();
                let _ = write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Self {
            endpoint,
            stop,
            accepted,
            thread: Some(thread),
            credit: Some(credit),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.thread.take().unwrap().join().unwrap();
        self.credit.take().unwrap().finish().unwrap();
    }
}

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
        let server = Server::start(cancel, matches!(case, "cancel" | "deadline"));
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
