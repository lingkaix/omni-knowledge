//! Native macOS smoke for the installed `omni-fsevents` sibling.
//!
//! `LocalMount::watch` resolves the helper beside this executable. The harness
//! does not pass `watch_with_helper` and does not read `CARGO_BIN_EXE_omni-fsevents`.
//! Linux returns 78 before opening a watch. That exit is not FSEvents evidence.

use std::env;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use knowledge_core::admission::{Account, Configuration, Scheduling};
#[cfg(target_os = "macos")]
use knowledge_core::backend::Budget;
#[cfg(target_os = "macos")]
use knowledge_local::{
    CurrentAdmission, LocalMount, LocalReadPolicy, LocalityPremise, ReadDecision,
};

fn main() -> ExitCode {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!(
            "omni-fsevents smoke requires native macOS; this process is not that observation"
        );
        return ExitCode::from(78);
    }
    #[cfg(target_os = "macos")]
    match run() {
        Ok(document) => {
            println!("{document}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(target_os = "macos")]
struct Owner;

#[cfg(target_os = "macos")]
impl LocalReadPolicy for Owner {
    fn decide(&self) -> ReadDecision {
        ReadDecision::Admit(CurrentAdmission {
            library_id: "packaging-fsevents".into(),
            mount_id: None,
            revision: 1,
            locality: LocalityPremise::OrdinaryLocal,
            outside_scope: vec![],
        })
    }
}

#[cfg(target_os = "macos")]
fn run() -> Result<String, String> {
    if env::var_os("CARGO_BIN_EXE_omni_fsevents").is_some() {
        return Err(
            "refusing CARGO_BIN_EXE_omni_fsevents; the sibling lookup must run".to_string(),
        );
    }
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let directory = exe
        .parent()
        .ok_or("harness executable has no parent directory")?;
    let helper = directory.join("omni-fsevents");
    if helper.is_symlink() || !helper.is_file() {
        return Err(format!(
            "omni-fsevents is not a regular file beside the harness at {}",
            helper.display()
        ));
    }
    let cache = directory.join("cache");
    let source = directory.join("source");
    fs::create_dir(&cache).map_err(|error| error.to_string())?;
    fs::create_dir(&source).map_err(|error| error.to_string())?;
    private_dir(&cache)?;
    private_dir(&source)?;
    let source = source.canonicalize().map_err(|error| error.to_string())?;
    let account = Account::start(
        File::open(&cache).map_err(|error| error.to_string())?,
        Configuration {
            memory_bytes: Some(128 * 1024 * 1024),
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 8,
            max_records: 32,
            max_descriptor_bytes: 65536,
            per_mount: 1,
        },
    )
    .map_err(|error| error.to_string())?;
    let base = account.status().map_err(|error| error.to_string())?;
    let mount = LocalMount::open(&source, "packaging-fsevents", None, Owner)
        .map_err(|error| format!("open: {error:?}"))?;
    let mut watch = mount
        .watch(budget())
        .map_err(|error| format!("watch: {error:?}"))?;
    let startup = watch
        .poll()
        .map_err(|error| format!("startup poll: {error:?}"))?;
    if !startup.coverage_complete || !startup.reconciliation_required {
        return Err("startup hint was not the public native coverage state".to_string());
    }
    fs::write(source.join("changed.txt"), b"actual native event")
        .map_err(|error| error.to_string())?;
    let started = Instant::now();
    loop {
        let hints = watch
            .poll()
            .map_err(|error| format!("event poll: {error:?}"))?;
        if hints.epoch > startup.epoch {
            if !hints.reconciliation_required {
                return Err("delivered hint did not require reconciliation".to_string());
            }
            break;
        }
        if started.elapsed() >= Duration::from_secs(5) {
            return Err("no actual FSEvents callback delivery".to_string());
        }
        thread_sleep();
    }
    let cursor = watch
        .native_cursor()
        .map_err(|error| format!("cursor: {error:?}"))?
        .ok_or("native cursor was absent after delivery")?;
    if cursor.event_id == 0 || cursor.volume == [0; 16] {
        return Err("native cursor did not carry an observed volume and event id".to_string());
    }
    let root = mount
        .root_identity()
        .map_err(|error| format!("root: {error:?}"))?;
    if cursor.root != root {
        return Err("native cursor root does not match the held mount".to_string());
    }
    watch.close().map_err(|error| format!("close: {error:?}"))?;
    let after = account.status().map_err(|error| error.to_string())?;
    if after.reserved.memory_bytes != base.reserved.memory_bytes || after.holdings != base.holdings
    {
        return Err(format!(
            "holdings were not settled after close: memory {} -> {}, holdings {} -> {}",
            base.reserved.memory_bytes, after.reserved.memory_bytes, base.holdings, after.holdings
        ));
    }
    Ok(serde_json::json!({
        "platform": "macos",
        "lookup": "sibling-omni-fsevents",
        "public_api": "LocalMount::watch",
        "startup_reconciliation_required": true,
        "event_delivered": true,
        "cursor_observed": true,
        "cursor_event_id_nonzero": true,
        "close": "confirmed",
        "holdings_settled": true,
        "child_pid_sample": "not-collected-no-proc-fallback",
        "child_settlement": "close-confirmed",
    })
    .to_string())
}

#[cfg(target_os = "macos")]
fn budget() -> Budget {
    Budget {
        max_files: 100_000,
        max_bytes: 16 * 1024 * 1024,
        max_millis: 15_000,
        max_memory_bytes: 16 * 1024 * 1024,
    }
}

#[cfg(target_os = "macos")]
fn private_dir(path: &Path) -> Result<(), String> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn thread_sleep() {
    std::thread::sleep(Duration::from_millis(20));
}
