//! Real Process sources and separately selected destination writes share the actual native owner.
use super::*;
use crate::native_service::embedding_tests::{digest, ready};
use crate::native_service::publication_tests::approve;
use knowledge_core::admission::Account;
use knowledge_core::backend::Budget;
use knowledge_core::jobs::{Claim, Priority, Reservation};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

const TEST_NAME: &str =
    "native_service::derived_publication_tests::actual_process_text_and_video_native_publication";
const TEXT: &str = "Native derived publication Ω\noriginal text survives\n";
const FIRST: &str = "first-selected.mp4";
const SECOND: &str = "second-selected.mp4";

/// Existing fixed media/text bounds are complete native work ceilings, never destination authority.
fn budget() -> Budget {
    Budget {
        max_files: 8,
        max_bytes: 16 * 1024 * 1024,
        max_millis: 60000,
        max_memory_bytes: 512 * 1024 * 1024,
    }
}
/// Actual local owner policy for two independently selected native roots.
fn policy() -> LocalPolicy {
    LocalPolicy {
        mode: wire::ExposureMode::ReadWrite,
        sensitive: false,
        excluded_paths: vec![],
        transports: vec![wire::Transport::OwnerStdio],
        http_hosts: vec![],
        http_origins: vec![],
    }
}
/// Use the wire catalog to admit the exact selected Process source and recipe.
fn command(value: serde_json::Value) -> wire::Command {
    wire::decode_request(
        &serde_json::to_vec(
            &serde_json::json!({"wire_version":1,"request_id":"native-derived","command":value}),
        )
        .unwrap(),
    )
    .unwrap()
    .command
}
/// Pin the actual separate parent inode and namespace; matching library labels do not select it.
fn destination(owner: &Owner, library: wire::Id, path: &Path) -> Selection {
    let path = path.to_owned();
    let selected = fs::symlink_metadata(&path).unwrap();
    let identity = (selected.dev(), selected.ino());
    owner
        .select_current(
            library,
            File::open(&path).unwrap(),
            policy(),
            Arc::new(move |held| {
                fs::symlink_metadata(&path)
                    .ok()
                    .zip(held.metadata().ok())
                    .is_some_and(|(named, held)| {
                        named.is_dir()
                            && !named.file_type().is_symlink()
                            && held.is_dir()
                            && (named.dev(), named.ino()) == identity
                            && (held.dev(), held.ino()) == identity
                    })
            }),
        )
        .unwrap()
}
/// The existing native channel presents the whole command before its actual owner response.
fn approved(
    service: &NativeLibraryService,
    owner: &Owner,
    session: &Session,
    command: &wire::Command,
) -> Permit {
    service
        .native_scope(
            session,
            command,
            Some(approve(owner, session, command, true).unwrap()),
        )
        .unwrap()
}
/// Produce the small deterministic real recording with the same installed mature media tool.
fn video(path: &Path) {
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x64:rate=25",
            "-t",
            "0.2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-threads",
            "1",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "synthetic actual video: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
/// Inspect the independent installed MP4 file rather than trusting a publication status or worker name.
fn assert_clip(path: &Path) {
    let bytes = fs::read(path).unwrap();
    assert!(bytes.len() > 8);
    assert_eq!(&bytes[4..8], b"ftyp");
}
/// Submit the opaque selected full recipe's complete processing/publication peak, then take its real Claim.
fn claim(
    processing: &NativeProcessing,
    plan: &NativePublicationPlan,
    video: bool,
    unknown: bool,
) -> Claim {
    let needed = plan
        .requirement(Reservation {
            memory_bytes: if video {
                512 * 1024 * 1024
            } else {
                64 * 1024 * 1024
            },
            temporary_bytes: if video { 8 * 1024 * 1024 } else { 0 },
        })
        .unwrap();
    let submitted = processing
        .submit(NativeJobPlan {
            operation_id: "derived-publication",
            job_id: "derived-publication",
            budget: Budget {
                max_millis: if unknown { 2000 } else { 60000 },
                ..budget()
            },
            reservation: needed,
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let claim = Account::current()
        .unwrap()
        .queue()
        .claim(Instant::now())
        .unwrap()
        .unwrap();
    assert_eq!(claim.job(), submitted.job);
    claim
}

/// Measure the same real two-clip recipe once under the original finite staging promise.
/// A later tighter promise distinguishes the last unknown writer from already reconciled clips.
#[cfg(target_os = "linux")]
fn calibration(service: &NativeLibraryService, permit: &Permit, relative: &str) -> u64 {
    let processing = service
        .source(
            permit,
            &wire::RelativePath::new(relative).unwrap(),
            budget(),
        )
        .unwrap()
        .processing(
            digest(b"actual video calibration private staging"),
            digest(include_bytes!("derived_publication_tests.rs")),
        )
        .unwrap();
    let submitted = processing
        .submit(NativeJobPlan {
            operation_id: "video-calibration",
            job_id: "video-calibration",
            budget: budget(),
            reservation: Reservation {
                memory_bytes: 512 * 1024 * 1024,
                temporary_bytes: 8 * 1024 * 1024 + 64 * 1024,
            },
            priority: Priority::Normal,
            max_attempts: 1,
        })
        .unwrap();
    let account = Account::current().unwrap();
    let claim = account.queue().claim(Instant::now()).unwrap().unwrap();
    assert_eq!(claim.job(), submitted.job);
    let clips = processing
        .segment_video(
            claim,
            NativeVideoPlan {
                seconds: 0.1,
                clips: knowledge_processing::media::ClipPlan {
                    child_rss_bytes: Some(256 * 1024 * 1024),
                    storage_bytes: 8 * 1024 * 1024,
                    max_segments: 2,
                },
            },
        )
        .unwrap_or_else(|error| panic!("actual paid calibration: {:#}", error.cause()));
    let value = clips.value().unwrap();
    assert_eq!(value.segments.len(), 2);
    let total: u64 = value
        .segments
        .iter()
        .map(|s| s.artifact.file.metadata().unwrap().len())
        .sum();
    assert!(total > 0 && total < 8 * 1024 * 1024);
    eprintln!(
        "actual two-clip calibration bytes={total}; unknown run uses this exact finite total promise"
    );
    drop(clips.complete().unwrap());
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    total
}

/// Locate the already built native parser before any owned delegation wrapper is selected.
#[cfg(target_os = "linux")]
fn parser_worker() -> std::path::PathBuf {
    for name in [
        "CARGO_BIN_EXE_parser_worker",
        "KNOWLEDGE_PROCESSING_PARSER_WORKER",
    ] {
        if let Some(path) = std::env::var_os(name) {
            let path = std::path::PathBuf::from(path);
            assert!(path.is_file());
            return path;
        }
    }
    let path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("parser-worker");
    assert!(path.is_file());
    path
}

/// Observe only this fixture's exact native PID/start/state receipt, never unrelated process data.
#[cfg(target_os = "linux")]
fn process_state(pid: u32) -> Option<(u64, char)> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    Some((
        fields.get(19)?.parse().ok()?,
        fields.first()?.chars().next()?,
    ))
}

/// Exact task-owned escaped pipe holder, stopped before foreground native error destruction.
#[cfg(target_os = "linux")]
struct Escape {
    pid: u32,
    start: u64,
    stopped: bool,
}
#[cfg(target_os = "linux")]
impl Escape {
    /// Independently qualify the native identity recorded by the owned wrapper.
    fn read() -> Self {
        let receipt: serde_json::Value = serde_json::from_slice(
            &fs::read(std::env::var_os("OMNI_DERIVED_ESCAPE").unwrap()).unwrap(),
        )
        .unwrap();
        let pid = u32::try_from(receipt["pid"].as_u64().unwrap()).unwrap();
        let start = receipt["start"].as_u64().unwrap();
        let actual = process_state(pid).unwrap();
        assert_eq!(actual.0, start);
        assert!(!matches!(actual.1, 'Z' | 'X'));
        Self {
            pid,
            start,
            stopped: false,
        }
    }
    /// Signal only the exact captured native identity; native error collection still proves settlement.
    fn stop(&mut self) -> anyhow::Result<()> {
        if self.stopped {
            return Ok(());
        }
        if let Some((start, state)) = process_state(self.pid) {
            anyhow::ensure!(start == self.start, "owned escaped PID identity changed");
            if !matches!(state, 'Z' | 'X') {
                let pid = rustix::process::Pid::from_raw(i32::try_from(self.pid)?)
                    .ok_or_else(|| anyhow::anyhow!("invalid owned PID"))?;
                rustix::process::kill_process(pid, rustix::process::Signal::KILL)?;
            }
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while process_state(self.pid)
            .is_some_and(|(start, state)| start == self.start && !matches!(state, 'Z' | 'X'))
        {
            anyhow::ensure!(
                Instant::now() < deadline,
                "owned escaped pipe process remains live"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        self.stopped = true;
        Ok(())
    }
}
#[cfg(target_os = "linux")]
impl Drop for Escape {
    /// Bound panic cleanup to the owned PID; it never settles the native resource owner itself.
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Actual escaped pipes, residual writer accounting and retryable foreign-member refusal.
#[cfg(target_os = "linux")]
fn unknown_video(
    built: std::result::Result<
        NativeAdmitted<knowledge_processing::held::HeldVideo>,
        NativeAdmittedError<knowledge_processing::held::HeldVideo>,
    >,
    mode: &str,
    output: &Path,
    original: &Path,
    maximum: u64,
) {
    let guard = Escape::read();
    let mut error = match built {
        Err(error) => error,
        Ok(_) => panic!("escaped native pipes cannot be a successful video result"),
    };
    let mut escaped = guard;
    assert!(
        error
            .cause()
            .is::<knowledge_processing::worker::UnsettledChild>(),
        "actual unknown native video owner: {:#}",
        error.cause()
    );
    let directory = error.staging_directory().unwrap().try_clone().unwrap();
    let source = fs::metadata(original).unwrap();
    let child = error
        .cause()
        .downcast_ref::<knowledge_processing::worker::UnsettledChild>()
        .unwrap();
    let files: Vec<_> = child
        .descriptors()
        .iter()
        .map(|f| f.metadata().unwrap())
        .filter(|m| m.is_file() && (m.dev(), m.ino()) != (source.dev(), source.ino()))
        .collect();
    assert_eq!(files.len(), 2);
    assert_eq!(
        files.iter().map(|m| m.len()).sum::<u64>(),
        maximum,
        "same real ffmpeg recipe must retain the independently calibrated exact extent"
    );
    let account = Account::current().unwrap();
    assert_eq!(
        fs::read_to_string(std::env::var_os("OMNI_DERIVED_COUNTER").unwrap()).unwrap(),
        "2"
    );
    let before = account.status().unwrap().reserved;
    assert_eq!(account.status().unwrap().queue.unsettled, 1);
    assert!(
        error.collect().is_err(),
        "leader exit/PID wait does not collect actual escaped pipes"
    );
    assert_eq!(account.status().unwrap().reserved, before);
    let extra = if mode == "video-unknown-foreign" {
        let mut file = File::from(
            rustix::fs::openat(
                &directory,
                "fixture-extra",
                rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::EXCL
                    | rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::from_bits_truncate(0o600),
            )
            .unwrap(),
        );
        std::io::Write::write_all(&mut file, b"known injected foreign member").unwrap();
        Some(file)
    } else {
        None
    };
    escaped.stop().unwrap();
    if let Some(extra) = extra {
        assert!(
            error.collect().is_err(),
            "actual stopped group does not authorize accounting a foreign member"
        );
        assert_eq!(
            account.status().unwrap().reserved,
            before,
            "foreign member refusal retains exact same native credit"
        );
        let held = extra.metadata().unwrap();
        let named = rustix::fs::statat(
            &directory,
            "fixture-extra",
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .unwrap();
        assert_eq!((named.st_dev, named.st_ino), (held.dev(), held.ino()));
        rustix::fs::unlinkat(&directory, "fixture-extra", rustix::fs::AtFlags::empty()).unwrap();
        drop(extra);
    }
    error.collect().unwrap_or_else(|cause| {
        panic!("same actual owner residual video reconciliation: {cause:#}")
    });
    error
        .collect()
        .unwrap_or_else(|cause| panic!("actual native collection must be idempotent: {cause:#}"));
    assert_eq!(
        account.status().unwrap().queue.unsettled,
        1,
        "native group/pipe/write collection is distinct from Claim settlement"
    );
    assert_eq!(fs::read_dir(output).unwrap().count(), 0);
    drop(directory);
    drop(error);
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
}
/// Isolate actual process accounts, run real text/clip producers, and observe their owning failures.
#[test]
fn actual_process_text_and_video_native_publication() {
    let Some(mode) = std::env::var_os("OMNI_DERIVED_PUBLICATION_MODE") else {
        let modes = vec![
            "text-normal",
            "text-wrong-source",
            "text-wrong-recipe",
            "text-pressure",
            "text-collision",
            "text-revoked",
            "text-installed-revoked",
            "video-normal",
            "video-pressure",
            "video-collision2",
            "video-installed2-revoked",
            "video-first-replaced",
        ];
        #[cfg(target_os = "linux")]
        let modes = {
            let mut modes = modes;
            modes.extend([
                "video-unknown-clip2",
                "video-unknown-finalverify",
                "video-unknown-foreign",
            ]);
            modes
        };
        for mode in modes {
            let cache = tempfile::tempdir().unwrap();
            fs::set_permissions(cache.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let mut child = Command::new(std::env::current_exe().unwrap());
            child
                .args(["--exact", TEST_NAME, "--nocapture", "--test-threads=1"])
                .env("OMNI_DERIVED_PUBLICATION_MODE", mode)
                .env("OMNI_CACHE_DIR", cache.path());
            #[cfg(target_os = "linux")]
            let faults = tempfile::tempdir().unwrap();
            #[cfg(target_os = "linux")]
            if mode.starts_with("video-unknown-") {
                let script = r#"#!/usr/bin/python3
import json, os, subprocess, sys
media = os.path.basename(sys.argv[0]) == 'ffmpeg'
real = os.environ['OMNI_DERIVED_REAL_FFMPEG' if media else 'OMNI_DERIVED_REAL_PARSER']
target_parser = os.environ['OMNI_DERIVED_PUBLICATION_MODE'] == 'video-unknown-finalverify'
eligible = (media and not target_parser and '-fd' in sys.argv) or (not media and target_parser and len(sys.argv) > 1 and sys.argv[1] == 'verify-held')
if not os.path.exists(os.environ['OMNI_DERIVED_ARMED']) or not eligible:
    os.execv(real, [real, *sys.argv[1:]])
counter = os.environ['OMNI_DERIVED_COUNTER']
try:
    with open(counter) as f:
        count = int(f.read()) + 1
except FileNotFoundError:
    count = 1
with open(counter, 'w') as f:
    f.write(str(count))
if count != 2:
    os.execv(real, [real, *sys.argv[1:]])
done = subprocess.run([real, *sys.argv[1:]], stdin=sys.stdin.buffer, stdout=sys.stdout, stderr=sys.stderr, close_fds=False)
if done.returncode != 0:
    sys.exit(done.returncode)
p = subprocess.Popen(['/bin/sleep', '60'], start_new_session=True, stdout=sys.stdout, stderr=sys.stderr, close_fds=True)
with open('/proc/%d/stat' % p.pid) as f:
    start = int(f.read().rsplit(')', 1)[1].split()[19])
with open(os.environ['OMNI_DERIVED_ESCAPE'], 'w') as f:
    json.dump({'pid':p.pid, 'start':start}, f)
sys.exit(0)
"#;
                let ffmpeg = faults.path().join("ffmpeg");
                let parser = faults.path().join("parser-worker-wrapper");
                for path in [&ffmpeg, &parser] {
                    fs::write(path, script).unwrap();
                    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
                }
                let native_ffmpeg = knowledge_processing::runtime::resolve_executable(
                    knowledge_processing::runtime::RuntimeTool::FFmpeg,
                )
                .unwrap();
                let mut paths = vec![faults.path().to_owned()];
                paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
                child
                    .env("PATH", std::env::join_paths(paths).unwrap())
                    .env("OMNI_DERIVED_REAL_FFMPEG", native_ffmpeg)
                    .env("OMNI_DERIVED_REAL_PARSER", parser_worker())
                    .env("OMNI_DERIVED_ARMED", faults.path().join("armed"))
                    .env("OMNI_DERIVED_COUNTER", faults.path().join("counter"))
                    .env("OMNI_DERIVED_ESCAPE", faults.path().join("escaped.json"))
                    .env("CARGO_BIN_EXE_parser_worker", &parser)
                    .env("KNOWLEDGE_PROCESSING_PARSER_WORKER", &parser);
            }
            let output = child.output().unwrap();
            assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
            assert!(
                output.status.success(),
                "actual derived {mode}: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    let mode = mode.to_str().unwrap();
    let is_video = mode.starts_with("video-");
    let is_unknown = mode.starts_with("video-unknown-");
    let fixture = tempfile::tempdir().unwrap();
    let home = fixture.path().join("home");
    let output = fixture.path().join("selected-output");
    fs::create_dir(&output).unwrap();
    let kb = KnowledgeBase::init(&home, "Synthetic native derived output").unwrap();
    let relative = if is_video {
        "raw/native.mp4"
    } else {
        "raw/native.txt"
    };
    let original_path = home.join(relative);
    if is_video {
        video(&original_path);
    } else {
        fs::write(&original_path, TEXT).unwrap();
    }
    let original = fs::read(&original_path).unwrap();
    let (service, owner, _source_selection) =
        NativeLibraryService::ordinary_local(kb, policy()).unwrap();
    let owner = Arc::new(owner);
    let source_session = service.connect(TransportFacts::OwnerStdio).unwrap();
    let selected = destination(&owner, service.library_id().clone(), &output);
    let destination_session = service
        .connect_selected(&selected, TransportFacts::OwnerStdio)
        .unwrap();
    let status =
        command(serde_json::json!({"op":"library_status","library_id":service.library_id()}));
    let status_permit = service
        .native_scope(&source_session, &status, None)
        .unwrap();
    let observed = service
        .source(
            &status_permit,
            &wire::RelativePath::new(relative).unwrap(),
            budget(),
        )
        .unwrap();
    let id = observed.id().clone();
    drop(observed);
    let process = command(
        serde_json::json!({"op":"process","library_id":service.library_id(),"source_ids":[id],
        "recipe":if is_video || mode == "text-wrong-recipe" { "media" } else { "extract" },
        "operation_id":"derived-publication","budget":{"max_files":8,"max_bytes":"16777216","max_millis":60000,"max_memory_bytes":"536870912"}}),
    );
    let source_permit = approved(&service, &owner, &source_session, &process);
    let destination_permit = approved(&service, &owner, &destination_session, &process);
    let source = service
        .source(
            if mode == "text-wrong-source" {
                &status_permit
            } else {
                &source_permit
            },
            &wire::RelativePath::new(relative).unwrap(),
            budget(),
        )
        .unwrap();
    let maximum = 8 * 1024 * 1024;
    #[cfg(target_os = "linux")]
    let maximum = if is_unknown {
        calibration(&service, &source_permit, relative)
    } else {
        maximum
    };
    let names = if is_video {
        vec![FIRST.into(), SECOND.into()]
    } else {
        vec!["selected.txt".into()]
    };
    let metadata =
        ready(NativePublicationPlan::derived_metadata_requirement(&source, &names).unwrap());
    let recipe = if is_video {
        NativeDerivedRecipe::Video(NativeVideoPlan {
            seconds: 0.1,
            clips: knowledge_processing::media::ClipPlan {
                child_rss_bytes: Some(256 * 1024 * 1024),
                storage_bytes: maximum,
                max_segments: 2,
            },
        })
    } else {
        NativeDerivedRecipe::Text {
            max_bytes: 1024 * 1024,
            child_rss_bytes: None,
        }
    };
    let planned = NativePublicationPlan::derived(
        &owner,
        &selected,
        destination_permit,
        &source,
        names,
        recipe,
        metadata,
    );
    if matches!(mode, "text-wrong-source" | "text-wrong-recipe") {
        assert!(
            planned.is_err(),
            "actual wrong Process/source intention cannot plan a destination write"
        );
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        assert_eq!(
            Account::current()
                .unwrap()
                .status()
                .unwrap()
                .queue
                .unsettled,
            0
        );
        return;
    }
    let mut plan = planned.unwrap();
    if mode == "text-installed-revoked" || mode == "video-installed2-revoked" {
        let owner = owner.clone();
        let selected = selected.clone();
        let index = usize::from(is_video);
        plan.test_after_install_at(index, move || owner.revoke(&selected).unwrap());
        plan.test_install_fault_at(index);
    }
    if mode == "video-first-replaced" {
        let first = output.join(FIRST);
        plan.test_after_install_at(1, move || {
            fs::rename(&first, first.with_extension("original")).unwrap();
            fs::write(&first, b"replacement actor").unwrap();
        });
    }
    let processing = source
        .processing(
            plan.recipe().clone(),
            digest(include_bytes!("derived_publication_tests.rs")),
        )
        .unwrap();
    let claim = claim(&processing, &plan, is_video, is_unknown);
    let account = Account::current().unwrap();
    let pressure = if mode.ends_with("pressure") {
        let held = ready(Reservation {
            memory_bytes: account.status().unwrap().memory_available,
            temporary_bytes: 0,
        });
        assert_eq!(account.status().unwrap().memory_available, 0);
        Some(held)
    } else {
        None
    };
    #[cfg(target_os = "linux")]
    if is_unknown {
        fs::write(
            std::env::var_os("OMNI_DERIVED_ARMED").unwrap(),
            b"actual paid fault run starts after complete calibration",
        )
        .unwrap();
    }
    if is_video {
        let built = processing.segment_video_planned(claim, plan);
        #[cfg(target_os = "linux")]
        if is_unknown {
            unknown_video(built, mode, &output, &original_path, maximum);
            assert_eq!(fs::read(&original_path).unwrap(), original);
            return;
        }
        let built = built
            .unwrap_or_else(|error| panic!("actual native video preparation: {:#}", error.cause()));
        let value = built.value().unwrap();
        assert_eq!(value.segments.len(), 2);
        assert_eq!(value.segments[0].start_seconds, 0.0);
        assert_eq!(
            value.segments[0].end_seconds,
            value.segments[1].start_seconds
        );
        assert!((value.segments[1].end_seconds - 0.2).abs() < 0.000001);
        if mode == "video-collision2" {
            fs::write(output.join(SECOND), b"existing destination").unwrap();
        }
        let published = built.publish();
        if matches!(mode, "video-collision2" | "video-installed2-revoked") {
            let mut error = match published {
                Err(error) => error,
                Ok(_) => panic!("actual partial/uncertain clip install requires its owning error"),
            };
            assert_clip(&output.join(FIRST));
            if mode == "video-collision2" {
                assert_eq!(
                    fs::read(output.join(SECOND)).unwrap(),
                    b"existing destination"
                );
            } else {
                assert_clip(&output.join(SECOND));
            }
            assert_eq!(account.status().unwrap().queue.unsettled, 1);
            error
                .collect()
                .unwrap_or_else(|cause| panic!("actual partial clip collection: {cause:#}"));
            assert_eq!(account.status().unwrap().queue.unsettled, 1);
            drop(error);
            assert_clip(&output.join(FIRST));
        } else if mode == "video-first-replaced" {
            let owner = match published {
                Err(mut error) => {
                    error.collect().unwrap();
                    drop(error);
                    None
                }
                Ok(owner) => Some(owner),
            };
            if let Some(owner) = owner {
                let error = match owner.complete() {
                    Err(error) => error,
                    Ok(_) => {
                        panic!("replaced first final inode cannot become all-output completion")
                    }
                };
                drop(error);
            }
            assert_eq!(fs::read(output.join(FIRST)).unwrap(), b"replacement actor");
            assert_clip(&output.join(SECOND));
        } else {
            let done = published
                .unwrap_or_else(|error| panic!("actual video publication: {:#}", error.cause()));
            assert_clip(&output.join(FIRST));
            assert_clip(&output.join(SECOND));
            done.complete()
                .unwrap_or_else(|error| panic!("actual video completion: {:#}", error.cause()))
                .check()
                .unwrap();
        }
    } else {
        let built = processing
            .extract_planned(claim, plan)
            .unwrap_or_else(|error| panic!("actual native text preparation: {:#}", error.cause()));
        assert_eq!(built.value().unwrap().text, TEXT);
        if mode == "text-collision" {
            fs::write(output.join("selected.txt"), b"existing destination").unwrap();
        }
        if mode == "text-revoked" {
            owner.revoke(&selected).unwrap();
        }
        let published = built.publish();
        if matches!(
            mode,
            "text-collision" | "text-revoked" | "text-installed-revoked"
        ) {
            let mut error = match published {
                Err(error) => error,
                Ok(_) => panic!(
                    "actual collision/revocation/uncertainty cannot report publication success"
                ),
            };
            assert_eq!(account.status().unwrap().queue.unsettled, 1);
            error
                .collect()
                .unwrap_or_else(|cause| panic!("actual text publication collection: {cause:#}"));
            assert_eq!(account.status().unwrap().queue.unsettled, 1);
            drop(error);
            match mode {
                "text-collision" => assert_eq!(
                    fs::read(output.join("selected.txt")).unwrap(),
                    b"existing destination"
                ),
                "text-revoked" => assert!(!output.join("selected.txt").exists()),
                _ => assert_eq!(
                    fs::read_to_string(output.join("selected.txt")).unwrap(),
                    TEXT
                ),
            }
        } else {
            let done = published
                .unwrap_or_else(|error| panic!("actual text publication: {:#}", error.cause()));
            assert_eq!(
                fs::read_to_string(output.join("selected.txt")).unwrap(),
                TEXT
            );
            done.complete()
                .unwrap_or_else(|error| panic!("actual text completion: {:#}", error.cause()))
                .check()
                .unwrap();
        }
    }
    drop(pressure);
    assert_eq!(account.status().unwrap().queue.unsettled, 0);
    assert_eq!(fs::read(&original_path).unwrap(), original);
    assert!(
        fs::read_dir(&output).unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_str()
            .unwrap()
            .starts_with(".omni-publication-")),
        "actual stage identity cleanup must settle before owner release"
    );
}
