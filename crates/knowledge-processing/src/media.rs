//! Bounded local media conversion and timestamp-preserving speech transcript parsing.

use crate::admission::{self, Charge};
use crate::runtime::{RuntimeTool, resolve_executable};
use crate::worker::{self, WorkerFailure};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Maximum transcript JSON size accepted for one source file.
const MAX_TRANSCRIPT_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum number of clips created by one segmentation operation.
const MAX_SEGMENTS: usize = 100_000;

/// One derived video clip with its position in the authoritative source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaSegment {
    /// Inclusive source position in seconds.
    pub start_seconds: f64,
    /// Exclusive source position in seconds, bounded by the source duration.
    pub end_seconds: f64,
    /// Derived MP4 file owned and cleaned by the caller.
    pub path: PathBuf,
}

/// Speech text and its location in the source recording.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// Inclusive source position in seconds; zero for a transcript without timing.
    pub start_seconds: f64,
    /// Exclusive source position in seconds; zero for a transcript without timing.
    pub end_seconds: f64,
    /// Nonempty text produced by speech recognition or an imported transcript.
    pub text: String,
}

/// Classifies common media extensions without opening, changing, or executing the file.
/// Returns `None` for other files, which remain eligible for general metadata indexing.
pub fn media_kind(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" | "tif" | "tiff" | "heic" | "heif"
        | "avif" | "svg" => Some("image"),
        "mp4" | "mov" | "m4v" | "mkv" | "avi" | "webm" | "mpg" | "mpeg" | "mts" | "m2ts" | "ts"
        | "3gp" => Some("video"),
        "wav" | "mp3" | "flac" | "ogg" | "opus" | "m4a" | "aac" | "aiff" | "aif" | "caf"
        | "wma" => Some("audio"),
        _ => None,
    }
}

/// Creates contiguous H.264 MP4 clips with exact source ranges in the caller's output directory.
/// The default caller duration is ten seconds; accepted durations are 0.1 through 600 seconds.
/// Requires executable `ffprobe` and `ffmpeg` at standard Homebrew locations or on PATH.
/// Never replaces an existing output file.
/// The caller owns cleanup of all created outputs, including partial work after an error.
pub fn segment_video(path: &Path, output_dir: &Path, seconds: f64) -> Result<Vec<MediaSegment>> {
    admission::with_legacy(None, || segment_video_body(path, output_dir, seconds))
}

/// Segment one video after the host binds the shared account and any child ceiling.
pub fn segment_video_admitted(
    path: &Path,
    output_dir: &Path,
    seconds: f64,
    account: &mut impl admission::SharedAccount,
    bound: admission::Bound,
) -> Result<Vec<MediaSegment>> {
    admission::with_host(account, bound, || {
        segment_video_body(path, output_dir, seconds)
    })
    .map_err(anyhow::Error::from)?
}

fn segment_video_body(path: &Path, output_dir: &Path, seconds: f64) -> Result<Vec<MediaSegment>> {
    ensure!(
        seconds.is_finite() && (0.1..=600.0).contains(&seconds),
        "Video segment duration must be from 0.1 through 600 seconds"
    );
    ensure!(
        output_dir.is_dir(),
        "Video output directory must already exist"
    );
    let source = fs::canonicalize(path)
        .with_context(|| format!("Cannot locate video {}", path.display()))?;
    ensure!(source.is_file(), "Video source must be a regular file");
    let duration = video_duration(&source)?;
    let count = (duration / seconds).ceil() as usize;
    ensure!(
        count <= MAX_SEGMENTS,
        "Video requires more than 100000 segments"
    );
    let ranges: Vec<_> = (0..count)
        .map(|index| {
            let start = index as f64 * seconds;
            (
                start,
                (start + seconds).min(duration),
                output_dir.join(format!("segment-{index:06}.mp4")),
            )
        })
        .collect();
    // Check every output first, so a collision cannot leave an earlier subset newly converted.
    for (_, _, output) in &ranges {
        ensure!(
            !output.try_exists()?,
            "Video clip already exists: {}",
            output.display()
        );
    }
    if let Some(bytes) = admission::storage_bytes() {
        admission::checkpoint()?;
        admission::before(Charge::Storage { bytes })?;
    }
    let mut segments = Vec::with_capacity(count);
    for (start, end, output) in ranges {
        let mut command = media_command(RuntimeTool::FFmpeg)?;
        command
            .args(["-nostdin", "-v", "error", "-ss"])
            .arg(format!("{start:.6}"))
            .arg("-i")
            .arg(&source)
            .arg("-t")
            .arg(format!("{:.6}", end - start))
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-vf",
                "setpts=PTS-STARTPTS,fps=25,scale=640:-2",
                "-c:v",
                "libx264",
                "-preset",
                "fast",
                "-crf",
                "25",
                "-pix_fmt",
                "yuv420p",
                "-n",
            ])
            .arg(&output);
        run_bounded(command, Duration::from_secs(120), "ffmpeg video conversion")?;
        ensure!(
            fs::metadata(&output)?.len() > 0,
            "ffmpeg produced an empty video clip"
        );
        segments.push(MediaSegment {
            start_seconds: start,
            end_seconds: end,
            path: output,
        });
    }
    Ok(segments)
}

/// Reads OpenAI Whisper JSON, whisper.cpp JSON, or a plain `{ "text": ... }` transcript.
/// Converts whisper.cpp millisecond offsets to seconds and rejects invalid or reversed ranges.
/// Plain text records use a zero-to-zero range because no timing evidence exists.
pub fn parse_transcript(path: &Path) -> Result<Vec<TranscriptSegment>> {
    admission::with_legacy(Some(MAX_TRANSCRIPT_BYTES), || parse_transcript_body(path))
}

/// Parse one transcript after the host binds the shared account.
pub fn parse_transcript_admitted(
    path: &Path,
    account: &mut impl admission::SharedAccount,
) -> Result<Vec<TranscriptSegment>> {
    admission::with_host(account, admission::Bound::default(), || {
        parse_transcript_body(path)
    })
    .map_err(anyhow::Error::from)?
}

fn parse_transcript_body(path: &Path) -> Result<Vec<TranscriptSegment>> {
    admission::checkpoint()?;
    admission::before(Charge::Input {
        bytes: MAX_TRANSCRIPT_BYTES,
    })?;
    admission::before(Charge::Live {
        bytes: MAX_TRANSCRIPT_BYTES,
    })?;
    let file =
        File::open(path).with_context(|| format!("Cannot read transcript {}", path.display()))?;
    ensure!(
        file.metadata()?.len() <= MAX_TRANSCRIPT_BYTES,
        "Transcript exceeds 16 MiB"
    );
    let mut bytes = Vec::new();
    file.take(MAX_TRANSCRIPT_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_TRANSCRIPT_BYTES,
        "Transcript exceeds 16 MiB"
    );
    let value: Value = serde_json::from_slice(&bytes).context("Transcript is not valid JSON")?;
    let mut result = Vec::new();
    if let Some(segments) = value.get("segments") {
        for segment in segments
            .as_array()
            .context("Transcript segments must be an array")?
        {
            result.push(transcript_segment(segment, false)?);
        }
    } else if let Some(segments) = value.get("transcription") {
        for segment in segments
            .as_array()
            .context("Transcript transcription must be an array")?
        {
            result.push(transcript_segment(segment, true)?);
        }
    } else if let Some(text) = value["text"].as_str() {
        result.push(TranscriptSegment {
            start_seconds: 0.0,
            end_seconds: 0.0,
            text: text.trim().into(),
        });
    } else {
        bail!("Transcript requires segments, transcription, or text");
    }
    result.retain(|segment| !segment.text.is_empty());
    Ok(result)
}

/// Converts audio to mono 16 kHz PCM WAV and invokes a configured local whisper.cpp executable.
/// Requires existing model and executable paths; it downloads no model and starts no service.
/// Produces `asr-input.wav` and `asr-result.json` in the caller's directory without replacing files.
/// The caller owns cleanup; recognition has a maximum duration of thirty minutes.
pub fn transcribe(
    path: &Path,
    whisper_executable: &Path,
    model: &Path,
    output_dir: &Path,
) -> Result<Vec<TranscriptSegment>> {
    admission::with_legacy(None, || {
        transcribe_body(path, whisper_executable, model, output_dir)
    })
}

/// Transcribe after the host binds the shared account and any child ceiling.
pub fn transcribe_admitted(
    path: &Path,
    whisper_executable: &Path,
    model: &Path,
    output_dir: &Path,
    account: &mut impl admission::SharedAccount,
    bound: admission::Bound,
) -> Result<Vec<TranscriptSegment>> {
    admission::with_host(account, bound, || {
        transcribe_body(path, whisper_executable, model, output_dir)
    })
    .map_err(anyhow::Error::from)?
}

fn transcribe_body(
    path: &Path,
    whisper_executable: &Path,
    model: &Path,
    output_dir: &Path,
) -> Result<Vec<TranscriptSegment>> {
    admission::checkpoint()?;
    ensure!(
        whisper_executable.is_file(),
        "Configured whisper.cpp executable is missing"
    );
    ensure!(model.is_file(), "Configured whisper.cpp model is missing");
    ensure!(
        output_dir.is_dir(),
        "Transcription output directory must already exist"
    );
    let source = fs::canonicalize(path).context("Cannot locate audio source")?;
    ensure!(source.is_file(), "Audio source must be a regular file");
    let wav = output_dir.join("asr-input.wav");
    let prefix = output_dir.join("asr-result");
    let output = output_dir.join("asr-result.json");
    ensure!(
        !wav.try_exists()? && !output.try_exists()?,
        "Transcription outputs already exist"
    );
    let mut converter = media_command(RuntimeTool::FFmpeg)?;
    converter
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(source)
        .args([
            "-map",
            "0:a:0",
            "-vn",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-c:a",
            "pcm_s16le",
            "-n",
        ])
        .arg(&wav);
    run_bounded(
        converter,
        Duration::from_secs(300),
        "ffmpeg audio conversion",
    )?;
    let mut recognizer = Command::new(whisper_executable);
    recognizer
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(&wav)
        .args(["-l", "auto", "-oj", "-of"])
        .arg(prefix);
    run_bounded(
        recognizer,
        Duration::from_secs(1800),
        "whisper.cpp speech recognition",
    )?;
    parse_transcript(&output)
}

/// Observes the first video stream and obtains a finite positive source duration.
fn video_duration(source: &Path) -> Result<f64> {
    let mut command = media_command(RuntimeTool::FFprobe)?;
    command
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_type,duration:format=duration",
            "-of",
            "json",
        ])
        .arg(source);
    let bytes = run_bounded(command, Duration::from_secs(30), "ffprobe video inspection")?;
    let value: Value = serde_json::from_slice(&bytes).context("ffprobe returned invalid JSON")?;
    let streams = value["streams"]
        .as_array()
        .context("Video stream is missing")?;
    ensure!(
        streams
            .first()
            .and_then(|stream| stream["codec_type"].as_str())
            == Some("video"),
        "Source has no video stream"
    );
    let duration = streams[0]["duration"]
        .as_str()
        .or_else(|| value["format"]["duration"].as_str())
        .context("Video duration is missing")?
        .parse::<f64>()
        .context("Video duration is invalid")?;
    ensure!(
        duration.is_finite() && duration > 0.0,
        "Video duration must be finite and positive"
    );
    Ok(duration)
}

/// Resolves only a typed media executable, including Homebrew tools when Finder has a limited PATH.
fn media_command(tool: RuntimeTool) -> Result<Command> {
    let path = resolve_executable(tool).with_context(|| {
        format!(
            "{} is missing; use the app's dependency guide to install the local tool",
            tool.executable()
        )
    })?;
    Ok(Command::new(path))
}

/// Parses and validates one timed transcript record in its producer's units.
fn transcript_segment(value: &Value, milliseconds: bool) -> Result<TranscriptSegment> {
    let (start, end) = if milliseconds {
        (
            value["offsets"]["from"].as_f64(),
            value["offsets"]["to"].as_f64(),
        )
    } else {
        (value["start"].as_f64(), value["end"].as_f64())
    };
    let divisor = if milliseconds { 1000.0 } else { 1.0 };
    let start = start.context("Transcript start time is missing")? / divisor;
    let end = end.context("Transcript end time is missing")? / divisor;
    ensure!(
        start.is_finite() && end.is_finite() && start >= 0.0 && end >= start,
        "Transcript time range is invalid"
    );
    let text = value["text"]
        .as_str()
        .context("Transcript segment text is missing")?
        .trim()
        .into();
    Ok(TranscriptSegment {
        start_seconds: start,
        end_seconds: end,
        text,
    })
}

/// Effective clip recipe recorded by the host in processor identity. Not a source digest.
pub fn clip_processor_facts() -> &'static [(&'static str, &'static str)] {
    &[
        ("codec", "libx264"),
        ("container", "mp4"),
        ("width", "640"),
        ("fps", "25"),
        ("audio", "none"),
        ("pixel_format", "yuv420p"),
    ]
}

/// Runs a local executable with a deadline, bounded captured output, and guaranteed child cleanup.
/// Commands receive separate arguments and a closed input stream; imported content is never executed.
fn run_bounded(mut command: Command, timeout: Duration, description: &str) -> Result<Vec<u8>> {
    admission::checkpoint()?;
    let rss_limit = admission::child_rss_bytes();
    if let Some(limit) = rss_limit {
        admission::before(Charge::Live { bytes: limit })?;
    }
    let mut stdout = tempfile::tempfile().context("Cannot allocate command output")?;
    let mut stderr = tempfile::tempfile().context("Cannot allocate command diagnostics")?;
    command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().with_context(|| {
        format!("Cannot start {description}; check local executable availability")
    })?;
    let deadline = admission::deadline().map_or_else(
        || Instant::now() + timeout,
        |host| host.min(Instant::now() + timeout),
    );
    let status = loop {
        let output_sizes = stdout
            .metadata()
            .and_then(|out| stderr.metadata().map(|err| (out.len(), err.len())));
        let (out_size, err_size) = match output_sizes {
            Ok(sizes) => sizes,
            Err(error) => {
                worker::settle(&mut child);
                return Err(error).with_context(|| format!("Cannot inspect {description} output"));
            }
        };
        if out_size > 1024 * 1024 || err_size > 1024 * 1024 {
            worker::settle(&mut child);
            return Err(anyhow::Error::new(WorkerFailure::OutputExceeded)
                .context(format!("{description} diagnostics exceed 1 MiB")));
        }
        if let Err(error) = admission::checkpoint() {
            worker::settle(&mut child);
            return Err(error.into());
        }
        if let Some(limit) = rss_limit {
            match worker::resident_bytes(child.id()) {
                Ok(observed) if observed > limit => {
                    worker::settle(&mut child);
                    return Err(anyhow::Error::new(WorkerFailure::RssExceeded {
                        limit_bytes: limit,
                        observed_bytes: observed,
                    }));
                }
                Ok(_) => {}
                Err(_) => match child.try_wait() {
                    Ok(Some(status)) => break status,
                    _ => {
                        worker::settle(&mut child);
                        return Err(anyhow::Error::new(WorkerFailure::Unmeasured));
                    }
                },
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                worker::settle(&mut child);
                return Err(anyhow::Error::new(WorkerFailure::Deadline).context(format!(
                    "{description} exceeded its {} second deadline",
                    timeout.as_secs()
                )));
            }
            Err(error) => {
                worker::settle(&mut child);
                return Err(error).with_context(|| format!("Cannot observe {description}"));
            }
        }
    };
    if !status.success() {
        stderr.seek(SeekFrom::Start(0))?;
        let mut diagnostic = String::new();
        stderr.take(4096).read_to_string(&mut diagnostic)?;
        bail!("{description} failed with {status}: {}", diagnostic.trim());
    }
    stdout.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new();
    stdout.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "{description} output exceeds 1 MiB"
    );
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::{AdmitError, Bound, Charge, SharedAccount};
    use crate::worker::WorkerFailure;
    use std::collections::HashSet;
    use std::sync::Mutex;
    use std::time::Instant;

    static SLEEP_GATE: Mutex<()> = Mutex::new(());

    struct Ledger {
        memory: u64,
        storage: u64,
        cancel: bool,
    }

    impl SharedAccount for Ledger {
        fn reserve(&mut self, charge: Charge) -> Result<(), AdmitError> {
            let need = charge.bytes();
            if charge.is_storage() {
                if self.storage < need {
                    return Err(AdmitError::TooLargeStorage);
                }
                self.storage -= need;
            } else if self.memory < need {
                return Err(AdmitError::TooLargeMemory);
            } else {
                self.memory -= need;
            }
            Ok(())
        }

        fn cancelled(&self) -> bool {
            self.cancel
        }
    }

    fn sleep_pids() -> HashSet<u32> {
        let me = std::process::id();
        let mut pids = HashSet::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return pids;
        };
        for entry in entries.flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
                continue;
            };
            let mut name = None;
            let mut parent = None;
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("Name:") {
                    name = rest.split_whitespace().next().map(str::to_string);
                }
                if let Some(rest) = line.strip_prefix("PPid:") {
                    parent = rest
                        .split_whitespace()
                        .next()
                        .and_then(|value| value.parse().ok());
                }
            }
            if parent == Some(me) && name.as_deref() == Some("sleep") {
                pids.insert(pid);
            }
        }
        pids
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_resident_sample_is_live_vmrss() {
        let observed = worker::resident_bytes(std::process::id()).expect("VmRSS");
        assert!(observed > 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rss_ceiling_kills_and_reaps_sleep() {
        let _gate = SLEEP_GATE.lock().expect("sleep gate");
        let before = sleep_pids();
        let mut ledger = Ledger {
            memory: u64::MAX,
            storage: 0,
            cancel: false,
        };
        let error = admission::with_host(
            &mut ledger,
            Bound {
                child_rss_bytes: Some(1),
                storage_bytes: None,
                deadline: None,
            },
            || {
                let mut command = Command::new("/bin/sleep");
                command.arg("30");
                run_bounded(command, Duration::from_secs(5), "sleep probe")
            },
        )
        .expect("binding")
        .expect_err("sleep must exceed one byte");
        assert!(error.chain().any(|item| {
            matches!(
                item.downcast_ref::<WorkerFailure>(),
                Some(WorkerFailure::RssExceeded { limit_bytes: 1, .. })
            )
        }));
        assert!(sleep_pids().is_subset(&before));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn deadline_reaps_sleep_without_a_new_memory_ceiling() {
        let _gate = SLEEP_GATE.lock().expect("sleep gate");
        let before = sleep_pids();
        let started = Instant::now();
        let error = admission::with_legacy(None, || {
            let mut command = Command::new("/bin/sleep");
            command.arg("30");
            run_bounded(command, Duration::from_millis(200), "sleep probe")
        })
        .expect_err("deadline");
        assert!(format!("{error}").contains("exceeded its"));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(sleep_pids().is_subset(&before));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn cancellation_does_not_start_the_child() {
        let _gate = SLEEP_GATE.lock().expect("sleep gate");
        let before = sleep_pids();
        let mut ledger = Ledger {
            memory: u64::MAX,
            storage: 0,
            cancel: true,
        };
        let error = admission::with_host(&mut ledger, Bound::default(), || {
            let mut command = Command::new("/bin/sleep");
            command.arg("30");
            run_bounded(command, Duration::from_secs(5), "sleep probe")
        })
        .expect("binding")
        .expect_err("cancelled");
        assert!(error.downcast_ref::<AdmitError>() == Some(&AdmitError::Cancelled));
        assert!(sleep_pids().is_subset(&before));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn refused_child_promise_does_not_spawn() {
        let _gate = SLEEP_GATE.lock().expect("sleep gate");
        let before = sleep_pids();
        let mut ledger = Ledger {
            memory: 0,
            storage: 0,
            cancel: false,
        };
        let error = admission::with_host(
            &mut ledger,
            Bound {
                child_rss_bytes: Some(1024),
                storage_bytes: None,
                deadline: None,
            },
            || {
                let mut command = Command::new("/bin/sleep");
                command.arg("30");
                run_bounded(command, Duration::from_secs(5), "sleep probe")
            },
        )
        .expect("binding")
        .expect_err("refused");
        assert!(error.downcast_ref::<AdmitError>() == Some(&AdmitError::TooLargeMemory));
        assert_eq!(sleep_pids(), before);
    }
}
