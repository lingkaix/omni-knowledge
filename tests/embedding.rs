//! Boundary regressions for local embeddings, video segmentation, and transcripts.

fn ensure_processing_account() {
    omni_knowledge::extract::establish_process_account().expect("process account");
}

use omni_knowledge::embedding::{
    EmbeddingClient, EmbeddingConfig, ProviderRequestError, cosine_similarity,
};
use omni_knowledge::media::{parse_transcript, segment_video, transcribe};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Owns an isolated loopback service and records requests without retaining input content.
struct MockService {
    /// Local URL assigned by the operating system.
    endpoint: String,
    /// Observed methods and paths.
    requests: Arc<Mutex<Vec<String>>>,
    /// Stops the server when the test completes.
    stop: Arc<AtomicBool>,
    /// Server thread joined during cleanup.
    thread: Option<JoinHandle<()>>,
}

impl MockService {
    /// Starts a JSON service with the supplied health and embedding replies.
    fn start(health: Value, embedding: Value, delay: Duration) -> Self {
        Self::with_models(health, models(), embedding, delay)
    }

    /// Starts a service with independently controlled advertised model identity.
    fn with_models(health: Value, models: Value, embedding: Value, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let observed = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let handle = thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0_u8; 1];
                while !request.ends_with(b"\r\n\r\n") {
                    if stream.read(&mut byte).unwrap_or(0) == 0 {
                        break;
                    }
                    request.push(byte[0]);
                }
                let headers = String::from_utf8_lossy(&request);
                let first = headers.lines().next().unwrap_or_default();
                let words: Vec<_> = first.split_whitespace().collect();
                if words.len() < 2 {
                    continue;
                }
                observed
                    .lock()
                    .unwrap()
                    .push(format!("{} {}", words[0], words[1]));
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                let mut discarded = vec![0; length];
                if length > 0 {
                    let _ = stream.read_exact(&mut discarded);
                    let payload: Value = serde_json::from_slice(&discarded).unwrap();
                    assert_eq!(payload["model"], EmbeddingConfig::default().model);
                    if let Some(content) = payload["messages"][0]["content"].as_array() {
                        assert_eq!(content.len(), 1);
                        let kind = content[0]["type"].as_str().unwrap();
                        assert!(matches!(kind, "image_url" | "video_url"));
                        assert!(
                            content[0][kind]["url"]
                                .as_str()
                                .unwrap()
                                .contains(";base64,")
                        );
                    } else {
                        assert!(payload["input"].is_string());
                    }
                }
                let value = match words[1] {
                    "/health" => &health,
                    "/v1/models" => &models,
                    "/v1/embeddings" => &embedding,
                    _ => panic!("Unexpected path: {}", words[1]),
                };
                thread::sleep(delay);
                let body = serde_json::to_vec(value).unwrap();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&body);
            }
        });
        Self {
            endpoint,
            requests,
            stop,
            thread: Some(handle),
        }
    }

    /// Returns a configuration directed only at this isolated service.
    fn config(&self) -> EmbeddingConfig {
        EmbeddingConfig {
            endpoint: self.endpoint.clone(),
            ..EmbeddingConfig::default()
        }
    }
}

impl Drop for MockService {
    /// Stops the isolated listener and joins its thread.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

/// Builds a compatible ready health record from the configured package identity.
fn health() -> Value {
    json!({"status": "ready", "backend": "mock", "package_fingerprint": EmbeddingConfig::default().package_fingerprint})
}

/// Builds the advertised model list independently from an embedding response.
fn models() -> Value {
    let config = EmbeddingConfig::default();
    json!({"data": [{"id": config.model, "dimension": config.dimension,
        "embedding_space": config.embedding_space, "modalities": ["text", "image", "video"]}]})
}

/// Builds one nonzero embedding with complete model and space identity.
fn embedding() -> Value {
    let config = EmbeddingConfig::default();
    let mut vector = vec![0.0; config.dimension];
    vector[0] = 1.0;
    json!({"model": config.model, "data": [{"index": 0, "embedding": vector}],
        "indexed": {"dimension": config.dimension, "embedding_space": config.embedding_space}})
}

/// Detects a package change before private document text can reach the service.
#[test]
fn incompatible_health_never_sends_content() {
    ensure_processing_account();
    let mut invalid = health();
    invalid["package_fingerprint"] = json!("a different package");
    let server = MockService::start(invalid, embedding(), Duration::ZERO);
    let client = EmbeddingClient::new(&server.config()).unwrap();
    let error = client.embed_text("Private document text").unwrap_err();
    assert!(error.downcast_ref::<ProviderRequestError>().is_some());
    assert_eq!(*server.requests.lock().unwrap(), ["GET /health"]);
}

/// Separates local unreadable or invalid input from fatal provider request failures.
#[test]
fn local_media_errors_do_not_mark_provider_failure() {
    ensure_processing_account();
    let server = MockService::start(health(), embedding(), Duration::ZERO);
    let client = EmbeddingClient::new(&server.config()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let error = client
        .embed_image(&directory.path().join("missing.png"))
        .unwrap_err();
    assert!(error.downcast_ref::<ProviderRequestError>().is_none());
    let error = client.embed_text("").unwrap_err();
    assert!(error.downcast_ref::<ProviderRequestError>().is_none());
    assert!(server.requests.lock().unwrap().is_empty());
}

/// Detects an advertised space or dimension change before sending document content.
#[test]
fn incompatible_model_list_never_sends_content() {
    ensure_processing_account();
    for (field, value) in [
        ("embedding_space", json!("different")),
        ("dimension", json!(64)),
    ] {
        let mut invalid = models();
        invalid["data"][0][field] = value;
        let server = MockService::with_models(health(), invalid, embedding(), Duration::ZERO);
        assert!(
            EmbeddingClient::new(&server.config())
                .unwrap()
                .embed_text("Private text")
                .is_err()
        );
        assert_eq!(
            *server.requests.lock().unwrap(),
            ["GET /health", "GET /v1/models"]
        );
    }
}

/// Confirms content crosses the boundary only after both identity observations pass.
#[test]
fn compatible_provider_returns_checked_vector() {
    ensure_processing_account();
    let server = MockService::start(health(), embedding(), Duration::ZERO);
    let client = EmbeddingClient::new(&server.config()).unwrap();
    let vector = client.embed_text("Retrieve the relevant image").unwrap();
    assert_eq!(vector.len(), EmbeddingConfig::default().dimension);
    assert_eq!(vector[0], 1.0);
    assert_eq!(
        *server.requests.lock().unwrap(),
        ["GET /health", "GET /v1/models", "POST /v1/embeddings"]
    );
}

/// Checks one visual input uses the exact image_url and video_url message keys.
#[test]
fn visual_requests_use_provider_message_contract() {
    ensure_processing_account();
    let directory = tempfile::tempdir().unwrap();
    let image = directory.path().join("fixture.png");
    let video = directory.path().join("fixture.mp4");
    std::fs::write(&image, b"synthetic image input").unwrap();
    std::fs::write(&video, b"synthetic video input").unwrap();
    let server = MockService::start(health(), embedding(), Duration::ZERO);
    let client = EmbeddingClient::new(&server.config()).unwrap();
    assert!(client.embed_image(&image).is_ok());
    assert!(client.embed_video(&video).is_ok());
    assert_eq!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.starts_with("POST"))
            .count(),
        2
    );
}

/// Rejects response-space changes even when vector dimensions remain equal.
#[test]
fn response_space_mismatch_is_rejected() {
    ensure_processing_account();
    let mut invalid = embedding();
    invalid["indexed"]["embedding_space"] = json!("a different space");
    let server = MockService::start(health(), invalid, Duration::ZERO);
    let client = EmbeddingClient::new(&server.config()).unwrap();
    assert!(client.embed_text("query").is_err());
}

/// Rejects zero, missing, and truncated vectors rather than publishing meaningless similarity.
#[test]
fn malformed_vectors_are_rejected() {
    ensure_processing_account();
    for vector in [
        json!([]),
        json!([1.0]),
        json!(vec![0.0; 2048]),
        json!(vec![Value::Null; 2048]),
    ] {
        let mut invalid = embedding();
        invalid["data"][0]["embedding"] = vector;
        let server = MockService::start(health(), invalid, Duration::ZERO);
        assert!(
            EmbeddingClient::new(&server.config())
                .unwrap()
                .embed_text("query")
                .is_err()
        );
    }
}

/// Restricts endpoints to literal loopback or canonical localhost and disables URL credentials.
#[test]
fn remote_and_credential_endpoints_are_rejected() {
    ensure_processing_account();
    for endpoint in [
        "http://example.com:18789",
        "http://192.168.1.1:18789",
        "http://user:secret@127.0.0.1:18789",
        "http://127.0.0.1:18789/?redirect=remote",
    ] {
        let config = EmbeddingConfig {
            endpoint: endpoint.into(),
            ..EmbeddingConfig::default()
        };
        assert!(EmbeddingClient::new(&config).is_err());
    }
}

/// Bounds waiting for an unresponsive local service.
#[test]
fn provider_timeout_is_bounded() {
    ensure_processing_account();
    let server = MockService::start(health(), embedding(), Duration::from_millis(1400));
    let mut config = server.config();
    config.timeout_seconds = 1;
    let start = Instant::now();
    assert!(EmbeddingClient::new(&config).unwrap().health().is_err());
    assert!(start.elapsed() < Duration::from_millis(1300));
}

/// Computes cosine for finite nonzero vectors and rejects invalid vector pairs.
#[test]
fn cosine_requires_compatible_finite_vectors() {
    ensure_processing_account();
    assert!(
        (cosine_similarity(&[2.0, 0.0], &[1.0, 1.0]).unwrap() - 2.0_f32.sqrt().recip()).abs()
            < 1e-6
    );
    assert!(cosine_similarity(&[0.0], &[1.0]).is_err());
    assert!(cosine_similarity(&[f32::NAN], &[1.0]).is_err());
    assert!(cosine_similarity(&[1.0], &[1.0, 2.0]).is_err());
}

/// Preserves second-based OpenAI Whisper ranges and millisecond-based whisper.cpp ranges.
#[test]
fn transcript_formats_preserve_time_ranges() {
    ensure_processing_account();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("transcript.json");
    std::fs::write(
        &path,
        r#"{"segments":[{"start":1.25,"end":3.75,"text":"OpenAI transcript"}]}"#,
    )
    .unwrap();
    let segments = parse_transcript(&path).unwrap();
    assert_eq!(
        (segments[0].start_seconds, segments[0].end_seconds),
        (1.25, 3.75)
    );
    std::fs::write(&path, r#"{"transcription":[{"offsets":{"from":1250,"to":3750},"text":"whisper.cpp transcript"}]}"#).unwrap();
    let segments = parse_transcript(&path).unwrap();
    assert_eq!(
        (segments[0].start_seconds, segments[0].end_seconds),
        (1.25, 3.75)
    );
    std::fs::write(
        &path,
        r#"{"segments":[{"start":3.0,"end":1.0,"text":"Invalid range"}]}"#,
    )
    .unwrap();
    assert!(parse_transcript(&path).is_err());
}

/// Exercises real audio normalization and the whisper.cpp CLI boundary with a deterministic recognizer.
/// This proves argument and timestamp handling, not speech-recognition accuracy.
#[cfg(unix)]
#[test]
fn transcription_normalizes_audio_and_parses_cli_result() {
    ensure_processing_account();
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("speech.flac");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            "0.25",
            "-ac",
            "2",
            "-n",
        ])
        .arg(&source)
        .status()
        .unwrap();
    assert!(status.success());
    let executable = directory.path().join("mock-whisper");
    std::fs::write(&executable, r#"#!/bin/sh
set -eu
input=''
prefix=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    -f) input="$2"; shift 2 ;;
    -of) prefix="$2"; shift 2 ;;
    -m|-l) shift 2 ;;
    -oj) shift ;;
    *) exit 11 ;;
  esac
done
format=$(ffprobe -v error -select_streams a:0 -show_entries stream=sample_rate,channels -of csv=p=0 "$input")
[ "$format" = '16000,1' ] || exit 12
printf '%s' '{"transcription":[{"offsets":{"from":0,"to":250},"text":"Recognized fixture"}]}' > "$prefix.json"
"#).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    let model = directory.path().join("model.bin");
    std::fs::write(&model, b"mock model boundary").unwrap();
    let outputs = directory.path().join("asr");
    std::fs::create_dir(&outputs).unwrap();
    let transcript = transcribe(&source, &executable, &model, &outputs).unwrap();
    assert_eq!(transcript[0].text, "Recognized fixture");
    assert_eq!(transcript[0].end_seconds, 0.25);
    assert!(transcribe(&source, &executable, &model, &outputs).is_err());
}

/// Verifies actual local recognition through the public conversion and transcript interface.
/// The fixture must contain the known utterance about private files and pineapple recipes.
#[cfg(target_os = "macos")]
#[test]
#[ignore = "Requires the pinned local ASR model and the generated speech fixture; run with --ignored after setup"]
fn actual_local_asr_recognizes_known_speech() {
    ensure_processing_account();
    let executable = std::env::var_os("OMNI_ASR_EXECUTABLE").expect("Set OMNI_ASR_EXECUTABLE");
    let model = std::env::var_os("OMNI_ASR_MODEL").expect("Set OMNI_ASR_MODEL");
    let fixture = std::env::var_os("OMNI_ASR_FIXTURE").expect("Set OMNI_ASR_FIXTURE");
    let output = tempfile::tempdir().unwrap();
    let segments = transcribe(
        std::path::Path::new(&fixture),
        std::path::Path::new(&executable),
        std::path::Path::new(&model),
        output.path(),
    )
    .unwrap();
    let text = segments
        .iter()
        .map(|item| item.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    assert!(
        text.contains("private"),
        "Known privacy sentence was not recognized"
    );
    assert!(
        text.contains("pineapple"),
        "Known retrieval term was not recognized"
    );
    assert!(
        text.contains("project notes"),
        "Known project sentence was not recognized"
    );
    assert!(!segments.is_empty());
    assert!(
        segments
            .iter()
            .all(|item| item.start_seconds >= 0.0 && item.end_seconds > item.start_seconds)
    );
}

/// Decodes actual ffmpeg clips and verifies complete coverage, including the final short range.
#[test]
fn ffmpeg_segments_cover_video_without_overwriting() {
    ensure_processing_account();
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=blue:size=16x16:rate=25",
            "-t",
            "20.4",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-n",
        ])
        .arg(&source)
        .status()
        .expect("ffmpeg is required for media integration verification");
    assert!(status.success());
    let outputs = directory.path().join("clips");
    std::fs::create_dir(&outputs).unwrap();
    let segments = segment_video(&source, &outputs, 10.0).unwrap();
    assert_eq!(segments.len(), 3);
    assert_eq!(
        (segments[0].start_seconds, segments[0].end_seconds),
        (0.0, 10.0)
    );
    assert_eq!(
        (segments[1].start_seconds, segments[1].end_seconds),
        (10.0, 20.0)
    );
    assert!((segments[2].end_seconds - 20.4).abs() < 0.01);
    for segment in &segments {
        let decoded = std::process::Command::new("ffmpeg")
            .args(["-nostdin", "-v", "error", "-xerror", "-i"])
            .arg(&segment.path)
            .args(["-f", "null", "-"])
            .status()
            .unwrap();
        assert!(decoded.success());
    }
    let retained = std::fs::read(&segments[0].path).unwrap();
    assert!(segment_video(&source, &outputs, 10.0).is_err());
    assert_eq!(std::fs::read(&segments[0].path).unwrap(), retained);
}
