//! Shared owned, finite, joined HTTP numeric fixture. It proves no model quality/native authority.
use knowledge_core::{
    data::ContentDigest,
    jobs::{Key, Queue, Reservation},
};
use knowledge_processing::{admission, embedding::EmbeddingConfig};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
pub(crate) struct Cancel {
    pub(crate) queue: Queue,
    pub(crate) job: u64,
    pub(crate) version: ContentDigest,
    pub(crate) recipe: ContentDigest,
    pub(crate) processor: ContentDigest,
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
pub(crate) struct Server {
    pub(crate) endpoint: String,
    stop: Arc<AtomicBool>,
    pub(crate) accepted: Arc<AtomicUsize>,
    pub(crate) seen: Arc<std::sync::Mutex<Option<String>>>,
    thread: Option<JoinHandle<()>>,
    credit: Option<knowledge_core::admission::Holding>,
}
impl Server {
    pub(crate) fn start(config: &EmbeddingConfig, cancel: Option<Cancel>, delay: bool) -> Self {
        let account = admission::current().unwrap();
        let credit = match account
            .reserve(
                Reservation {
                    memory_bytes: 1024 * 1024,
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
        let model = config.model.clone();
        let fingerprint = config.package_fingerprint.clone();
        let space = config.embedding_space.clone();
        let dimension = config.dimension;
        let seen = Arc::new(std::sync::Mutex::new(None));
        let observed = seen.clone();
        let thread = thread::Builder::new().stack_size(256*1024).spawn(move || {
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
                let body=if first.starts_with("GET /health "){serde_json::json!({"status":"ready","package_fingerprint":fingerprint,"backend":"deterministic-contract-only"})}
    else if first.starts_with("GET /v1/models "){serde_json::json!({"data":[{"id":model,"dimension":dimension,"embedding_space":space}]})}
    else {
     if let Some(cancel)=&cancel{cancel.cancel();}
     if delay {let until=Instant::now()+Duration::from_secs(1);while Instant::now()<until&&!stopped.load(Ordering::Acquire){thread::sleep(Duration::from_millis(2));}}
     let supplied:serde_json::Value=serde_json::from_slice(&request[header_end..]).unwrap();let input=supplied["input"].as_str().unwrap();let n=input.len();*observed.lock().unwrap()=Some(input.to_owned());
     serde_json::json!({"model":model,"indexed":{"dimension":dimension,"embedding_space":space},"data":[{"index":0,"embedding":[n as f64,1.0]}]})
    }.to_string();
                let _ = write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        }).unwrap();
        Self {
            endpoint,
            stop,
            accepted,
            seen,
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
