//! Builder smoke for the real local adapter against disposable files.
//! This is not an independent acceptance test. Expectations come from the storage owner:
//! confined ordinary names, no symlink follow, stat without a body read, full-file digest
//! refusal when bytes outside the requested range differ, current admission, and budgets.
use knowledge_core::backend::{
    Availability, BackendError, Budget, ByteRange, Enumeration, EnumerationStop, ExpectedVersion,
    Freshness, MountBackend, Residency, Unavailable,
};
use knowledge_core::data::BackendKind;
use knowledge_core::data::{ContentDigest, RelativePath};
use knowledge_local::{
    CurrentAdmission, LocalError, LocalMount, LocalReadPolicy, LocalityPremise, ReadDecision,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixed(ReadDecision);

impl LocalReadPolicy for Fixed {
    fn decide(&self) -> ReadDecision {
        self.0.clone()
    }
}

struct Flip {
    calls: AtomicU64,
    admission: CurrentAdmission,
}

impl LocalReadPolicy for Flip {
    fn decide(&self) -> ReadDecision {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        let mut admission = self.admission.clone();
        admission.revision = call;
        ReadDecision::Admit(admission)
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok local-read-smoke");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("fail {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("knowledge-local-smoke-{}", std::process::id()));
    let _cleanup = Cleanup(root.clone());
    fs::create_dir_all(&root).map_err(show)?;
    let outside = root.join("outside-marker.bin");
    fs::write(&outside, vec![0x5a; 65_536]).map_err(show)?;
    let clean = root.join("clean");
    fs::create_dir_all(clean.join("dir")).map_err(show)?;
    fs::create_dir_all(clean.join("hidden")).map_err(show)?;
    fs::write(clean.join("note.txt"), b"hello world!!!!").map_err(show)?;
    fs::write(clean.join("dir").join("nested.txt"), b"nested").map_err(show)?;
    fs::write(clean.join("hidden").join("secret.txt"), b"secret-bytes").map_err(show)?;
    let large = clean.join("large.bin");
    write_len(&large, 1_048_576)?;

    let ordinary = admission("library-smoke", Some("mount-smoke"), &["hidden"]);
    let mount = LocalMount::open(
        &clean,
        "library-smoke",
        Some("mount-smoke".to_string()),
        Fixed(ReadDecision::Admit(ordinary)),
    )
    .map_err(show)?;
    expect(mount.capabilities().range_read, "range reads are supported")?;
    expect(
        !mount.capabilities().stable_file_id,
        "inode is not a stable id",
    )?;
    expect(
        !mount.capabilities().reliable_watch,
        "watch is not implemented",
    )?;
    expect(
        !mount.capabilities().conditional_write,
        "conditional write is not implemented",
    )?;
    expect(
        !mount.capabilities().residency,
        "local premise is not residency detection",
    )?;

    let listed = mount.list(None, generous()).map_err(show)?;
    let Enumeration::Complete { entries, .. } = listed else {
        return Err("clean tree did not list completely".to_string());
    };
    expect(
        paths(&entries).iter().any(|path| path == "note.txt"),
        "note listed",
    )?;
    expect(
        paths(&entries).iter().any(|path| path == "dir"),
        "directory listed",
    )?;
    expect(
        paths(&entries).iter().any(|path| path == "dir/nested.txt"),
        "nested file listed",
    )?;
    expect(
        !paths(&entries)
            .iter()
            .any(|path| path.starts_with("hidden")),
        "exclusion omitted",
    )?;

    let note = rel("note.txt")?;
    #[cfg(target_os = "linux")]
    {
        let before = read_counter()?;
        let large_stat = mount.stat(&rel("large.bin")?, generous()).map_err(show)?;
        let after = read_counter()?;
        expect(large_stat.snapshot.is_some(), "large file has metadata")?;
        expect(
            after.saturating_sub(before) < 262_144,
            "stat did not read the large body",
        )?;
    }
    let stat = mount.stat(&note, generous()).map_err(show)?;
    let snapshot = stat.snapshot.ok_or("note stat missing")?;
    expect(snapshot.size_bytes == 15, "stat reports logical size")?;
    expect(
        snapshot.residency == Residency::Unknown,
        "residency stays unknown",
    )?;
    expect(
        snapshot.freshness == Freshness::Current,
        "fresh stat is current",
    )?;
    expect(
        snapshot.backend.backend == BackendKind::Local,
        "backend kind is local",
    )?;
    expect(snapshot.backend_version.is_none(), "no provider version")?;

    let digest = sha256(b"hello world!!!!")?;
    let version = ExpectedVersion {
        digest: digest.clone(),
        snapshot: snapshot.clone(),
    };
    let opened = mount
        .open_range(&version, ByteRange::new(0, 5).map_err(show)?, generous())
        .map_err(show)?;
    expect(
        opened.bytes == b"hello",
        "range returns only the requested bytes",
    )?;
    expect(!opened.end_of_file, "short range is not eof")?;

    let mut changed = fs::read(clean.join("note.txt")).map_err(show)?;
    let last = changed.len() - 1;
    changed[last] = b'?';
    fs::write(clean.join("note.txt"), &changed).map_err(show)?;
    let new_stat = mount
        .stat(&note, generous())
        .map_err(show)?
        .snapshot
        .ok_or("new stat")?;
    let stale = ExpectedVersion {
        digest,
        snapshot: new_stat,
    };
    match mount.open_range(&stale, ByteRange::new(0, 5).map_err(show)?, generous()) {
        Err(BackendError::Unavailable(Unavailable::Changed)) => {}
        other => return Err(format!("outside-range change returned {other:?}")),
    }

    let linked = root.join("linked");
    fs::create_dir_all(&linked).map_err(show)?;
    fs::write(linked.join("inside.txt"), b"inside").map_err(show)?;
    std::os::unix::fs::symlink(&outside, linked.join("escape")).map_err(show)?;
    let link_mount = LocalMount::open(
        &linked,
        "library-smoke",
        Some("mount-smoke".to_string()),
        Fixed(ReadDecision::Admit(admission(
            "library-smoke",
            Some("mount-smoke"),
            &[],
        ))),
    )
    .map_err(show)?;
    match link_mount.list(None, generous()).map_err(show)? {
        Enumeration::Partial {
            entries,
            stopped: EnumerationStop::Unsupported,
            ..
        } => {
            expect(
                !paths(&entries).iter().any(|path| path == "escape"),
                "symlink is not an ordinary entry",
            )?;
        }
        other => return Err(format!("symlink tree returned {other:?}")),
    }
    let escape = rel("escape")?;
    let escape_stat = link_mount.stat(&escape, generous()).map_err(show)?;
    expect(
        matches!(
            escape_stat.availability,
            Availability::Unavailable(Unavailable::Unsupported)
        ),
        "symlink stat is unsupported",
    )?;
    #[cfg(target_os = "linux")]
    {
        let forged = ExpectedVersion {
            digest: sha256(b"inside")?,
            snapshot: link_mount
                .stat(&rel("inside.txt")?, generous())
                .map_err(show)?
                .snapshot
                .ok_or("inside")?,
        };
        let mut wrong = forged.clone();
        wrong.snapshot.path = escape;
        let before = read_counter()?;
        let result = link_mount.open_range(&wrong, ByteRange::new(0, 4).map_err(show)?, generous());
        let after = read_counter()?;
        expect(result.is_err(), "symlink range is refused")?;
        expect(
            after.saturating_sub(before) < 65_536,
            "symlink range did not read the outside file",
        )?;
    }

    let budget = Budget {
        max_files: 1,
        ..generous()
    };
    match mount.list(None, budget).map_err(show)? {
        Enumeration::Partial {
            entries,
            stopped: EnumerationStop::BudgetExhausted,
            ..
        } => {
            expect(entries.len() <= 1, "file budget retains at most one entry")?;
        }
        other => return Err(format!("file budget returned {other:?}")),
    }
    match mount
        .list(
            None,
            Budget {
                max_files: 0,
                ..generous()
            },
        )
        .map_err(show)?
    {
        Enumeration::Partial {
            entries,
            stopped: EnumerationStop::BudgetExhausted,
            ..
        } => {
            expect(
                entries.is_empty(),
                "zero file budget does not claim entries",
            )?;
        }
        other => return Err(format!("zero budget returned {other:?}")),
    }

    let odd = root.join("odd");
    fs::create_dir_all(&odd).map_err(show)?;
    let raw = std::ffi::OsString::from_vec(vec![0xff, 0xfe]);
    File::create(odd.join(raw)).map_err(show)?;
    let odd_mount = LocalMount::open(
        &odd,
        "library-smoke",
        Some("mount-smoke".to_string()),
        Fixed(ReadDecision::Admit(admission(
            "library-smoke",
            Some("mount-smoke"),
            &[],
        ))),
    )
    .map_err(show)?;
    match odd_mount.list(None, generous()).map_err(show)? {
        Enumeration::Complete { .. } => {
            return Err("non-utf8 name became a complete listing".to_string());
        }
        Enumeration::Partial {
            stopped: EnumerationStop::Unsupported,
            ..
        } => {}
        other => return Err(format!("non-utf8 listing returned {other:?}")),
    }

    let denied = LocalMount::open(
        &clean,
        "library-smoke",
        Some("mount-smoke".to_string()),
        Fixed(ReadDecision::Deny),
    )
    .map_err(show)?;
    match mount.stat(&rel("hidden")?, generous()) {
        Err(BackendError::Unavailable(Unavailable::Denied)) => {}
        other => return Err(format!("excluded stat returned {other:?}")),
    }
    match mount.stat(&rel("hidden/secret.txt")?, generous()) {
        Err(BackendError::Unavailable(Unavailable::Denied)) => {}
        other => return Err(format!("excluded child stat returned {other:?}")),
    }
    match denied.list(None, generous()) {
        Err(BackendError::Unavailable(Unavailable::Denied)) => {}
        other => return Err(format!("deny returned {other:?}")),
    }
    let mut unestablished = admission("library-smoke", Some("mount-smoke"), &[]);
    unestablished.locality = LocalityPremise::NotEstablished;
    let premise = LocalMount::open(
        &clean,
        "library-smoke",
        Some("mount-smoke".to_string()),
        Fixed(ReadDecision::Admit(unestablished)),
    )
    .map_err(show)?;
    match premise.list(None, generous()) {
        Err(BackendError::Unavailable(Unavailable::Unknown)) => {}
        other => return Err(format!("unestablished locality returned {other:?}")),
    }
    let unknown = LocalMount::open(
        &clean,
        "library-smoke",
        Some("mount-smoke".to_string()),
        Fixed(ReadDecision::Unestablished),
    )
    .map_err(show)?;
    match unknown.list(None, generous()) {
        Err(BackendError::Unavailable(Unavailable::Unknown)) => {}
        other => return Err(format!("unestablished returned {other:?}")),
    }

    let empty = root.join("empty");
    fs::create_dir_all(&empty).map_err(show)?;
    let empty_mount = LocalMount::open(
        &empty,
        "library-smoke",
        Some("mount-smoke".to_string()),
        Fixed(ReadDecision::Admit(admission(
            "library-smoke",
            Some("mount-smoke"),
            &[],
        ))),
    )
    .map_err(show)?;
    match empty_mount.list(None, generous()).map_err(show)? {
        Enumeration::Complete { entries, .. } if entries.is_empty() => {}
        other => return Err(format!("empty directory returned {other:?}")),
    }
    let flip = LocalMount::open(
        &empty,
        "library-smoke",
        Some("mount-smoke".to_string()),
        Flip {
            calls: AtomicU64::new(1),
            admission: admission("library-smoke", Some("mount-smoke"), &[]),
        },
    )
    .map_err(show)?;
    match flip.list(None, generous()) {
        Err(BackendError::Unavailable(Unavailable::Changed)) => {}
        other => return Err(format!("changed admission returned {other:?}")),
    }

    let cursor = knowledge_core::backend::Cursor {
        backend: snapshot.backend.clone(),
        token: "restart".to_string(),
    };
    match empty_mount.list(Some(&cursor), generous()) {
        Err(BackendError::InvalidRequest) => {}
        other => return Err(format!("cursor returned {other:?}")),
    }
    #[cfg(target_os = "linux")]
    {
        let special = root.join("special");
        fs::create_dir_all(&special).map_err(show)?;
        let dir = File::open(&special).map_err(show)?;
        rustix::fs::mkfifoat(
            &dir,
            "pipe",
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .map_err(show)?;
        let special_mount = LocalMount::open(
            &special,
            "library-smoke",
            Some("mount-smoke".to_string()),
            Fixed(ReadDecision::Admit(admission(
                "library-smoke",
                Some("mount-smoke"),
                &[],
            ))),
        )
        .map_err(show)?;
        match special_mount.list(None, generous()).map_err(show)? {
            Enumeration::Partial {
                stopped: EnumerationStop::Unsupported,
                ..
            } => {}
            other => return Err(format!("fifo listing returned {other:?}")),
        }
        let pipe_stat = special_mount
            .stat(&rel("pipe")?, generous())
            .map_err(show)?;
        expect(
            matches!(
                pipe_stat.availability,
                Availability::Unavailable(Unavailable::Unsupported)
            ),
            "fifo stat is unsupported",
        )?;
    }
    match LocalMount::open(
        &root.join("missing"),
        "library-smoke",
        None,
        Fixed(ReadDecision::Deny),
    ) {
        Err(LocalError::InvalidRoot) => {}
        Err(error) => return Err(format!("missing root returned {error}")),
        Ok(_) => return Err("missing root was bound".to_string()),
    }
    let alias = root.join("alias");
    std::os::unix::fs::symlink(&clean, &alias).map_err(show)?;
    match LocalMount::open(&alias, "library-smoke", None, Fixed(ReadDecision::Deny)) {
        Err(LocalError::InvalidRoot) => {}
        Err(error) => return Err(format!("symlink root returned {error}")),
        Ok(_) => return Err("symlink root was bound".to_string()),
    }
    let _ = (outside, large);
    Ok(())
}

fn admission(library: &str, mount: Option<&str>, outside: &[&str]) -> CurrentAdmission {
    CurrentAdmission {
        library_id: library.to_string(),
        mount_id: mount.map(str::to_string),
        revision: 7,
        locality: LocalityPremise::OrdinaryLocal,
        outside_scope: outside
            .iter()
            .map(|path| RelativePath::new(*path).expect("scope"))
            .collect(),
    }
}

fn generous() -> Budget {
    Budget {
        max_files: 10_000,
        max_bytes: 16 * 1_048_576,
        max_millis: 30_000,
        max_memory_bytes: 16 * 1_048_576,
    }
}

fn rel(path: &str) -> Result<RelativePath, String> {
    RelativePath::new(path).map_err(show)
}

fn sha256(bytes: &[u8]) -> Result<ContentDigest, String> {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        text.push(char::from(b"0123456789abcdef"[(byte & 0xf) as usize]));
    }
    ContentDigest::new(text).map_err(show)
}

fn paths(entries: &[knowledge_core::backend::ListedEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|entry| entry.path.as_str().to_string())
        .collect()
}

fn write_len(path: &Path, len: usize) -> Result<(), String> {
    let mut file = File::create(path).map_err(show)?;
    file.write_all(&vec![0u8; len]).map_err(show)
}

fn expect(condition: bool, message: &str) -> Result<(), String> {
    condition.then_some(()).ok_or_else(|| message.to_string())
}

fn show(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(target_os = "linux")]
fn read_counter() -> Result<u64, String> {
    let text = fs::read_to_string("/proc/self/io").map_err(show)?;
    text.lines()
        .find_map(|line| line.strip_prefix("read_bytes: "))
        .and_then(|value| value.trim().parse().ok())
        .ok_or_else(|| "read_bytes missing".to_string())
}

struct Cleanup(PathBuf);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
