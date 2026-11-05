//! Bounded local media conversion and timestamp-preserving speech transcript parsing.

pub use crate::held::segment_video_held_funded;

use crate::admission::{self, Admitted};
use crate::funding::{Credits, Funding, read_fd, selected_volume};
use crate::runtime::{RuntimeTool, resolve_executable};
use crate::worker::{self, WorkerFailure};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::File;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// Maximum transcript JSON size accepted for one source file.
const MAX_TRANSCRIPT_BYTES: u64 = 16 * 1024 * 1024;
/// Maximum number of clips created by one segmentation operation.
pub(crate) const MAX_SEGMENTS: usize = 100_000;

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

/// Creates contiguous H.264 MP4 clips using the compatibility pre-verification path.
pub fn segment_video(path: &Path, output_dir: &Path, seconds: f64) -> Result<Vec<MediaSegment>> {
    admission::current()?;
    let file =
        File::open(path).with_context(|| format!("Cannot locate video {}", path.display()))?;
    let storage = compatibility_storage()?;
    let result = segment_video_file_funded(
        &file,
        output_dir,
        seconds,
        ClipPlan {
            child_rss_bytes: None,
            storage_bytes: storage,
            max_segments: MAX_SEGMENTS,
        },
        &mut Funding::BeforeVerification,
    );
    Ok(worker::retain_source(result, file)?.value)
}

/// Complete bounds supplied by the job owner before any conversion effect.
#[derive(Clone, Copy, Debug)]
pub struct ClipPlan {
    /// Opaque process-group RSS promise; None spends only credit left after all buffers.
    pub child_rss_bytes: Option<u64>,
    /// Total generated extent promise on the selected cache volume.
    pub storage_bytes: u64,
    /// Maximum clip count, at most the existing 100000 hostile-input ceiling.
    pub max_segments: usize,
}

/// Compatibility descriptor API. Use the funded variant to retain its returned path-buffer credit.
pub fn segment_video_file(
    file: &File,
    output_dir: &Path,
    seconds: f64,
    child_rss_bytes: u64,
    storage_bytes: u64,
) -> Result<Vec<MediaSegment>> {
    Ok(segment_video_file_funded(
        file,
        output_dir,
        seconds,
        ClipPlan {
            child_rss_bytes: Some(child_rss_bytes),
            storage_bytes,
            max_segments: MAX_SEGMENTS,
        },
        &mut Funding::BeforeVerification,
    )?
    .value)
}

/// Segment an admitted current source with all concurrent costs prepaid, including the returned paths.
pub fn segment_video_file_funded(
    file: &File,
    output_dir: &Path,
    seconds: f64,
    plan: ClipPlan,
    funding: &mut Funding<'_>,
) -> Result<Admitted<Vec<MediaSegment>>> {
    admission::current()?;
    funding.checkpoint()?;
    ensure!(
        seconds.is_finite() && (0.1..=600.0).contains(&seconds),
        "Video segment duration must be from 0.1 through 600 seconds"
    );
    ensure!(
        plan.max_segments > 0 && plan.max_segments <= MAX_SEGMENTS,
        "Invalid clip count bound"
    );
    ensure!(
        output_dir.is_dir(),
        "Video output directory must already exist"
    );
    ensure!(
        file.metadata()?.is_file(),
        "Video source must be a regular file"
    );
    selected_volume(&File::open(output_dir)?)?;
    let first = output_dir.join("segment-000000.mp4");
    ensure!(
        !first.try_exists()?,
        "Video clip already exists: {}",
        first.display()
    );
    ensure!(
        plan.storage_bytes > 0,
        "Generated storage promise must be positive"
    );
    // Two arrays may coexist: complete collision-checked ranges and the returned segments.
    let path_bytes = output_dir
        .as_os_str()
        .len()
        .checked_add(128)
        .context("Clip path bound overflow")?;
    let result_bytes = (plan.max_segments as u64)
        .checked_mul((path_bytes + std::mem::size_of::<MediaSegment>()) as u64)
        .and_then(|n| n.checked_mul(2))
        .context("Clip memory bound overflow")?;
    let mut credits = Credits::new(
        funding,
        0,
        result_bytes,
        crate::opaque::capture_memory(1024 * 1024),
        plan.child_rss_bytes,
        plan.storage_bytes,
    )?;
    let _source_fd = worker::InheritedFd::new(file);
    let source = fd_path(file);
    let duration = video_duration(&source, &mut credits, funding)?;
    let count = (duration / seconds).ceil() as usize;
    ensure!(
        count <= plan.max_segments,
        "Video requires more than the admitted segment count"
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
    for (_, _, output) in &ranges {
        ensure!(
            !output.try_exists()?,
            "Video clip already exists: {}",
            output.display()
        );
    }
    let mut segments = Vec::with_capacity(count);
    for (start, end, output) in ranges {
        // The only regular output inode is ours before exec and is bound to the selected volume.
        let destination = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&output)?;
        selected_volume(&destination)?;
        let _output_fd = worker::InheritedFd::new(&destination);
        let limit = credits
            .storage
            .0
            .as_ref()
            .context("Missing generated storage promise")?
            .remaining()?
            .temporary_bytes;
        let mut command = media_command(RuntimeTool::FFmpeg)?;
        command
            .args(["-nostdin", "-v", "error", "-ss"])
            .arg(format!("{start:.6}"))
            .arg("-i")
            .arg(&source);
        clip_encoding(&mut command, end - start);
        command.arg(fd_path(&destination));
        let converted = run_command(
            command,
            Duration::from_secs(120),
            "ffmpeg video conversion",
            &mut credits,
            funding,
            Some(limit),
        );
        drop(_output_fd);
        let (length, _destination) =
            reconcile_writer(converted, destination, &mut credits, &[&output])?;
        ensure!(length > 0, "ffmpeg produced an empty video clip");
        segments.push(MediaSegment {
            start_seconds: start,
            end_seconds: end,
            path: output,
        });
    }
    let retained = credits.retained();
    Ok(Admitted {
        value: segments,
        retained,
    })
}

/// Reads OpenAI Whisper JSON, whisper.cpp JSON, or a plain `{ "text": ... }` transcript.
/// Converts whisper.cpp millisecond offsets to seconds and rejects invalid or reversed ranges.
/// Plain text records use a zero-to-zero range because no timing evidence exists.
pub fn parse_transcript(path: &Path) -> Result<Vec<TranscriptSegment>> {
    Ok(parse_transcript_path(path)?.value)
}

/// Parse one transcript and return the holding that covers the retained text.
pub fn parse_transcript_path(path: &Path) -> Result<Admitted<Vec<TranscriptSegment>>> {
    admission::current()?;
    let file =
        File::open(path).with_context(|| format!("Cannot read transcript {}", path.display()))?;
    worker::retain_source(parse_transcript_file(&file), file)
}

/// Parse an already admitted transcript with the compatibility pre-verification path.
pub fn parse_transcript_file(file: &File) -> Result<Admitted<Vec<TranscriptSegment>>> {
    parse_transcript_file_funded(file, None, &mut Funding::BeforeVerification)
}

/// Parse bounded descriptor bytes in the real watched parser, retaining result credit until freed.
pub fn parse_transcript_file_funded(
    file: &File,
    child_rss_bytes: Option<u64>,
    funding: &mut Funding<'_>,
) -> Result<Admitted<Vec<TranscriptSegment>>> {
    admission::current()?;
    funding.checkpoint()?;
    let length = file.metadata()?.len();
    ensure!(length <= MAX_TRANSCRIPT_BYTES, "Transcript exceeds 16 MiB");
    let capacity = usize::try_from(length)?
        .checked_add(1)
        .context("Transcript input overflow")?;
    let cap = crate::opaque::transport_cap(capacity);
    let output = cap.checked_mul(8).context("Transcript result overflow")?;
    let mut credits = Credits::new(
        funding,
        capacity as u64,
        output,
        crate::opaque::capture_memory(cap),
        child_rss_bytes,
        0,
    )?;
    let bytes = read_fd(file, capacity)?;
    ensure!(
        bytes.len() as u64 <= length,
        "Transcript grew during bounded descriptor read"
    );
    let result = crate::opaque::transcript_in_worker(&bytes, capacity, &mut credits, funding)?;
    drop(bytes);
    credits.input.release_now()?;
    let retained = credits.retained();
    Ok(Admitted {
        value: result,
        retained,
    })
}

/// Mature transcript parser used only inside the already funded RSS-watched process.
/// Retains OpenAI, whisper.cpp, plain text, millisecond conversion, and source-clock validation.
pub fn parse_transcript_bytes(bytes: &[u8]) -> Result<Vec<TranscriptSegment>> {
    let value: Value = serde_json::from_slice(bytes).context("Transcript is not valid JSON")?;
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

/// Leave native observation headroom in the compatibility estimate; funded callers choose their exact admitted extent.
fn compatibility_storage() -> Result<u64> {
    let available = admission::available_storage()?;
    Ok(available.saturating_sub((available / 100).max(1)))
}

/// Complete ASR bounds, chosen before creating any WAV/JSON output.
#[derive(Clone, Copy, Debug)]
pub struct AsrPlan {
    /// Opaque child-group RSS promise; None spends only credit left after buffers.
    pub child_rss_bytes: Option<u64>,
    /// Total new regular-file extent on the selected cache volume.
    pub storage_bytes: u64,
    /// Maximum transcript input bytes, no greater than the retained 16 MiB ceiling.
    pub transcript_bytes: usize,
}

/// Convert audio and run a configured local recognizer; the compatibility result keeps a stranded live charge.
pub fn transcribe(
    path: &Path,
    whisper_executable: &Path,
    model: &Path,
    output_dir: &Path,
) -> Result<Vec<TranscriptSegment>> {
    admission::current()?;
    let file = File::open(path).context("Cannot locate audio source")?;
    let storage = compatibility_storage()?;
    let result = transcribe_file_funded(
        &file,
        whisper_executable,
        model,
        output_dir,
        AsrPlan {
            child_rss_bytes: None,
            storage_bytes: storage,
            transcript_bytes: MAX_TRANSCRIPT_BYTES as usize,
        },
        &mut Funding::BeforeVerification,
    );
    Ok(worker::retain_source(result, file)?.value)
}

/// Compatibility descriptor API; the funded API consumes the current job's existing credits.
pub fn transcribe_file(
    file: &File,
    whisper_executable: &Path,
    model: &Path,
    output_dir: &Path,
    child_rss_bytes: u64,
    storage_bytes: u64,
) -> Result<Admitted<Vec<TranscriptSegment>>> {
    transcribe_file_funded(
        file,
        whisper_executable,
        model,
        output_dir,
        AsrPlan {
            child_rss_bytes: Some(child_rss_bytes),
            storage_bytes,
            transcript_bytes: MAX_TRANSCRIPT_BYTES as usize,
        },
        &mut Funding::BeforeVerification,
    )
}

/// Perform real conversion/recognition under prepaid extent ceilings, then parse within the same funded call.
pub fn transcribe_file_funded(
    file: &File,
    whisper_executable: &Path,
    model: &Path,
    output_dir: &Path,
    plan: AsrPlan,
    funding: &mut Funding<'_>,
) -> Result<Admitted<Vec<TranscriptSegment>>> {
    admission::current()?;
    funding.checkpoint()?;
    ensure!(
        whisper_executable.is_file(),
        "Configured whisper.cpp executable is missing"
    );
    ensure!(model.is_file(), "Configured whisper.cpp model is missing");
    ensure!(
        output_dir.is_dir(),
        "Transcription output directory must already exist"
    );
    ensure!(
        file.metadata()?.is_file(),
        "Audio source must be a regular file"
    );
    ensure!(
        plan.transcript_bytes > 0 && plan.transcript_bytes as u64 <= MAX_TRANSCRIPT_BYTES,
        "Invalid transcript byte bound"
    );
    selected_volume(&File::open(output_dir)?)?;
    ensure!(
        plan.storage_bytes > 0,
        "Generated storage promise must be positive"
    );
    let cap = crate::opaque::transport_cap(plan.transcript_bytes);
    let mut credits = Credits::new(
        funding,
        plan.transcript_bytes as u64 + 1,
        cap.checked_mul(8).context("ASR output overflow")?,
        crate::opaque::capture_memory(cap.max(1024 * 1024)),
        plan.child_rss_bytes,
        plan.storage_bytes,
    )?;
    let _inherited = worker::InheritedFd::new(file);
    let wav = output_dir.join("asr-input.wav");
    let prefix = output_dir.join("asr-result");
    let output = output_dir.join("asr-result.json");
    ensure!(
        !wav.try_exists()? && !output.try_exists()?,
        "Transcription outputs already exist"
    );
    let destination = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&wav)?;
    selected_volume(&destination)?;
    let _output_fd = worker::InheritedFd::new(&destination);
    let mut converter = media_command(RuntimeTool::FFmpeg)?;
    converter
        .args(["-nostdin", "-v", "error", "-i"])
        .arg(fd_path(file))
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
            "-f",
            "wav",
            "-y",
        ])
        .arg(fd_path(&destination));
    let limit = credits
        .storage
        .0
        .as_ref()
        .context("Missing storage promise")?
        .remaining()?
        .temporary_bytes;
    let converted = run_command(
        converter,
        Duration::from_secs(300),
        "ffmpeg audio conversion",
        &mut credits,
        funding,
        Some(limit),
    );
    drop(_output_fd);
    let (_, _wav_fd) = reconcile_writer(converted, destination, &mut credits, &[&wav])?;
    // The private admitted directory must remain exclusively owned while whisper uses its required basename protocol.
    let json = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&output)?;
    selected_volume(&json)?;
    let mut recognizer = Command::new(whisper_executable);
    recognizer
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(&wav)
        .args(["-l", "auto", "-oj", "-of"])
        .arg(prefix);
    let limit = credits
        .storage
        .0
        .as_ref()
        .context("Missing storage promise")?
        .remaining()?
        .temporary_bytes
        .min(plan.transcript_bytes as u64);
    let recognized = run_command(
        recognizer,
        Duration::from_secs(1800),
        "whisper.cpp speech recognition",
        &mut credits,
        funding,
        Some(limit),
    );
    let (length, json) = reconcile_writer(recognized, json, &mut credits, &[&wav, &output])?;
    ensure!(
        length <= plan.transcript_bytes as u64,
        "Transcript exceeds admitted bytes"
    );
    let bytes = read_fd(&json, plan.transcript_bytes + 1)?;
    ensure!(
        bytes.len() as u64 <= length,
        "Transcript changed during bounded read"
    );
    let segments =
        crate::opaque::transcript_in_worker(&bytes, plan.transcript_bytes, &mut credits, funding)?;
    drop(bytes);
    credits.input.release_now()?;
    let retained = credits.retained();
    Ok(Admitted {
        value: segments,
        retained,
    })
}

/// Observes the first video stream and obtains a finite positive source duration.
fn video_duration(source: &str, credits: &mut Credits, funding: &Funding<'_>) -> Result<f64> {
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
    let bytes = run_command(
        command,
        Duration::from_secs(30),
        "ffprobe video inspection",
        credits,
        funding,
        None,
    )?;
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
pub(crate) fn media_command(tool: RuntimeTool) -> Result<Command> {
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

/// Apply the single retained visual conversion recipe to either admitted input/output protocol.
pub(crate) fn clip_encoding(command: &mut Command, duration: f64) {
    command.arg("-t").arg(format!("{duration:.6}")).args([
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
        "-f",
        "mp4",
        "-y",
    ]);
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

/// Reconcile one new inode exactly once, including known failed partial output; unknown writers keep their promises and fixtures.
fn reconcile_writer(
    result: Result<Vec<u8>>,
    destination: File,
    credits: &mut Credits,
    artifacts: &[&Path],
) -> Result<(u64, File)> {
    let result = result.map(drop);
    if let Err(mut error) = result {
        if let Some(pending) = error.downcast_mut::<worker::UnsettledChild>() {
            for path in artifacts {
                pending.retain_artifact(path.to_path_buf());
            }
            pending.retain_descriptor(destination);
        } else {
            crate::funding::reconcile_file(destination, &mut credits.storage, artifacts)?;
        }
        return Err(error);
    }
    crate::funding::reconcile_file(destination, &mut credits.storage, artifacts)
}

/// All real media tools share the same continuously observed nonblocking runner.
fn run_command(
    command: Command,
    timeout: Duration,
    description: &str,
    credits: &mut Credits,
    funding: &Funding<'_>,
    extent: Option<u64>,
) -> Result<Vec<u8>> {
    let result = worker::run(
        command,
        None,
        1024 * 1024,
        timeout,
        credits,
        funding,
        extent,
    )
    .map(|capture| capture.bytes);
    if result
        .as_ref()
        .is_err_and(|e| e.downcast_ref::<WorkerFailure>() == Some(&WorkerFailure::Deadline))
    {
        return result.with_context(|| {
            format!(
                "{description} exceeded its {} second deadline",
                timeout.as_secs()
            )
        });
    }
    result.with_context(|| description.to_owned())
}

/// Existing private regression seam: actual tools use precisely the same admitted runner.
#[cfg(test)]
fn run_bounded(
    command: Command,
    timeout: Duration,
    description: &str,
    rss_limit: u64,
) -> Result<Vec<u8>> {
    if timeout.is_zero() {
        bail!(WorkerFailure::Deadline);
    }
    let mut funding = Funding::BeforeVerification;
    let mut credits = Credits::new(
        &mut funding,
        0,
        0,
        crate::opaque::capture_memory(1024 * 1024),
        Some(rss_limit),
        0,
    )?;
    run_command(command, timeout, description, &mut credits, &funding, None)
}

fn fd_path(file: &File) -> String {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        // The child resolves the parent's descriptor, not the caller's path.
        format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd())
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::io::AsRawFd;
        format!("/dev/fd/{}", file.as_raw_fd())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = file;
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::AdmitError;
    use crate::worker::WorkerFailure;
    use knowledge_core::admission::{Account, Configuration, Scheduling};
    use std::collections::HashSet;
    use std::sync::Once;
    use std::time::Instant;

    use crate::admission::TEST_GATE as SLEEP_GATE;

    fn ensure_account() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            use std::os::unix::fs::PermissionsExt;
            let path = tempfile::tempdir().unwrap().keep();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            let directory = File::open(&path).unwrap();
            match Account::start(
                directory,
                Configuration {
                    memory_bytes: None,
                    storage_bytes: None,
                },
                Scheduling {
                    max_pending: 64,
                    max_records: 4096,
                    max_descriptor_bytes: 1 << 20,
                    per_mount: 4,
                },
            ) {
                Ok(_) | Err(knowledge_core::admission::Error::AlreadyStarted) => {}
                Err(error) => panic!("start account: {error}"),
            }
        });
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
        ensure_account();
        let _gate = SLEEP_GATE.lock().expect("sleep gate");
        let before = sleep_pids();
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let error = run_bounded(command, Duration::from_secs(5), "sleep probe", 1)
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
        ensure_account();
        let _gate = SLEEP_GATE.lock().expect("sleep gate");
        let before = sleep_pids();
        let started = Instant::now();
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let error = run_bounded(
            command,
            Duration::from_millis(200),
            "sleep probe",
            64 * 1024 * 1024,
        )
        .expect_err("deadline");
        assert!(format!("{error}").contains("exceeded its"));
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(sleep_pids().is_subset(&before));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn expired_deadline_does_not_start_the_child() {
        let _gate = SLEEP_GATE.lock().expect("sleep gate");
        let before = sleep_pids();
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let error =
            run_bounded(command, Duration::ZERO, "sleep probe", 1024).expect_err("deadline");
        assert!(error.downcast_ref::<WorkerFailure>() == Some(&WorkerFailure::Deadline));
        assert!(sleep_pids().is_subset(&before));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn refused_child_promise_does_not_spawn() {
        ensure_account();
        let _gate = SLEEP_GATE.lock().expect("sleep gate");
        let before = sleep_pids();
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let error = run_bounded(command, Duration::from_secs(5), "sleep probe", u64::MAX)
            .expect_err("refused");
        assert!(error.downcast_ref::<AdmitError>() == Some(&AdmitError::TooLargeMemory));
        assert_eq!(sleep_pids(), before);
    }

    /// Direct-child exit must not release a still-running descendant's memory or keep its pipe writer alive.
    #[cfg(target_os = "linux")]
    #[test]
    fn direct_exit_collects_the_actual_owned_background_process() {
        ensure_account();
        let _gate = SLEEP_GATE.lock().unwrap_or_else(|e| e.into_inner());
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30 & printf '%s' \"$!\""]);
        let bytes = run_bounded(
            command,
            Duration::from_secs(4),
            "owned group exit probe",
            64 * 1024 * 1024,
        )
        .unwrap();
        let pid: u32 = std::str::from_utf8(&bytes).unwrap().trim().parse().unwrap();
        let live = std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
            s.rsplit_once(')')
                .and_then(|(_, rest)| rest.split_whitespace().next())
                .is_some_and(|state| state != "Z" && state != "X")
        });
        // On the reviewed implementation this exact fixture survives. Stop only its native PID before reporting RED.
        if live {
            assert!(
                Command::new("/bin/kill")
                    .args(["-KILL", &pid.to_string()])
                    .status()
                    .unwrap()
                    .success()
            );
        }
        assert!(
            !live,
            "direct exit left actual owned descendant {pid} running"
        );
    }
}
