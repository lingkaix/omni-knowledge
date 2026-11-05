//! Packaging check for installed HTML and EPUB extraction.
//!
//! Starts one process account on an owned private directory descriptor and reads
//! owned HTML and EPUB through `extract_admitted` and `read_epub_admitted`.
//! When `parser-worker` is beside this executable, Linux must observe that child.
//! This trunk's processing crate does not spawn that child; an absent file is not
//! replaced with a fabricated pid. The root CLI is not started.

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use knowledge_core::admission::{
    Account, Configuration, Decision, Error as AdmissionError, Holding, Origin, Scheduling, Waiting,
};
use knowledge_processing::admission::{AdmitError, Charge, SharedAccount, reservation};
use knowledge_processing::epub::read_epub_admitted;
use knowledge_processing::extract::extract_admitted;

fn main() -> ExitCode {
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

fn run() -> Result<String, String> {
    if env::var_os("CARGO_BIN_EXE_parser_worker").is_some()
        || env::var_os("KNOWLEDGE_PROCESSING_PARSER_WORKER").is_some()
    {
        return Err(
            "lookup override is set; refusing a bypass of the installed parser-worker".to_string(),
        );
    }
    let exe = env::current_exe().map_err(|error| error.to_string())?;
    let directory = exe
        .parent()
        .ok_or("harness executable has no parent directory")?
        .to_path_buf();
    let helper = directory.join("parser-worker");
    let helper_present = helper.is_file();
    if helper.is_symlink() {
        return Err("parser-worker beside the harness is a symlink".to_string());
    }
    let helper = if helper_present {
        Some(helper.canonicalize().map_err(|error| error.to_string())?)
    } else {
        None
    };
    let sample = Arc::new(Sample {
        stop: AtomicBool::new(false),
        pids: Mutex::new(Vec::new()),
        helper: helper.clone(),
    });
    let watcher = start_watcher(Arc::clone(&sample));
    let result = exercise(&directory);
    sample.stop.store(true, Ordering::Relaxed);
    if let Some(watcher) = watcher {
        watcher
            .join()
            .map_err(|_| "pid sampler thread panicked".to_string())?;
    }
    let admitted = result?;
    let pids = sample
        .pids
        .lock()
        .map_err(|_| "pid sampler lock failed")?
        .clone();
    let reaped = match &helper {
        Some(path) => pids.iter().all(|pid| !still_our_helper(*pid, path)),
        None => pids.is_empty(),
    };
    if helper.is_some() && cfg!(target_os = "linux") && pids.is_empty() {
        return Err("parser child pid was not observed; refusing to invent one".to_string());
    }
    if helper.is_none() && !pids.is_empty() {
        return Err(format!(
            "observed a parser pid without an installed helper: {pids:?}"
        ));
    }
    if !reaped {
        return Err(format!("parser child still running: {pids:?}"));
    }
    let sample_kind = if helper.is_some() && cfg!(target_os = "linux") {
        "proc-exe"
    } else if helper.is_some() {
        "not-linux"
    } else {
        "in-process"
    };
    Ok(serde_json::json!({
        "account": "started",
        "cache_fd": "owned-directory",
        "lookup_basename": if helper.is_some() { Some("parser-worker") } else { None::<&str> },
        "public_api": ["extract_admitted", "read_epub_admitted"],
        "html_status": admitted.html_status,
        "html_contains": admitted.html_contains,
        "epub_contains": admitted.epub_contains,
        "holdings_finished": true,
        "holdings_after": admitted.holdings_after,
        "child_pids": pids,
        "children_reaped": reaped,
        "child_pid_sample": sample_kind,
    })
    .to_string())
}

struct AdmittedText {
    html_status: String,
    html_contains: bool,
    epub_contains: bool,
    holdings_after: usize,
}

struct Sample {
    stop: AtomicBool,
    pids: Mutex<Vec<u32>>,
    helper: Option<PathBuf>,
}

fn start_watcher(sample: Arc<Sample>) -> Option<thread::JoinHandle<()>> {
    if sample.helper.is_none() || !cfg!(target_os = "linux") {
        return None;
    }
    Some(thread::spawn(move || {
        while !sample.stop.load(Ordering::Relaxed) {
            observe(&sample);
            thread::yield_now();
        }
        observe(&sample);
    }))
}

fn observe(sample: &Sample) {
    let Some(helper) = &sample.helper else {
        return;
    };
    let Ok(entries) = fs::read_dir("/proc") else {
        return;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|text| text.parse::<u32>().ok())
        else {
            continue;
        };
        if !still_our_helper(pid, helper) {
            continue;
        }
        let Ok(mut pids) = sample.pids.lock() else {
            return;
        };
        if !pids.contains(&pid) {
            pids.push(pid);
        }
    }
}

fn still_our_helper(pid: u32, helper: &Path) -> bool {
    let Ok(exe) = fs::read_link(format!("/proc/{pid}/exe")) else {
        return false;
    };
    exe.canonicalize().ok().as_deref() == Some(helper)
}

struct Host {
    holdings: Vec<Holding>,
    fault: String,
}

impl SharedAccount for Host {
    fn reserve(&mut self, charge: Charge) -> Result<(), AdmitError> {
        let account = match Account::current() {
            Ok(account) => account,
            Err(error) => {
                self.fault = error.to_string();
                return Err(AdmitError::Busy);
            }
        };
        match account.reserve(reservation(charge), Origin::BeforeVerification) {
            Ok(Decision::Ready(holding)) => {
                self.holdings.push(holding);
                Ok(())
            }
            Ok(Decision::Waiting(Waiting::Memory)) => Err(AdmitError::WaitingMemory),
            Ok(Decision::Waiting(Waiting::Storage)) => Err(AdmitError::WaitingStorage),
            Err(error @ AdmissionError::TooLargeMemory) => {
                self.fault = error.to_string();
                Err(AdmitError::TooLargeMemory)
            }
            Err(error @ AdmissionError::TooLargeStorage) => {
                self.fault = error.to_string();
                Err(AdmitError::TooLargeStorage)
            }
            Err(error) => {
                self.fault = error.to_string();
                Err(AdmitError::Busy)
            }
        }
    }
}

fn exercise(directory: &Path) -> Result<AdmittedText, String> {
    let cache = directory.join("cache");
    fs::create_dir(&cache).map_err(|error| error.to_string())?;
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o700))
        .map_err(|error| error.to_string())?;
    let cache_fd = File::open(&cache).map_err(|error| error.to_string())?;
    let account = Account::start(
        cache_fd,
        Configuration {
            memory_bytes: Some(512 * 1024 * 1024),
            storage_bytes: None,
        },
        Scheduling {
            max_pending: 64,
            max_records: 4096,
            max_descriptor_bytes: 1 << 20,
            per_mount: 4,
        },
    )
    .map_err(|error| error.to_string())?;
    let base_holdings = account
        .status()
        .map_err(|error| error.to_string())?
        .holdings;
    let html_path = directory.join("page.html");
    fs::write(&html_path, b"<p>Hello packaging</p>").map_err(|error| error.to_string())?;
    let mut html_status = String::new();
    let mut html_contains = false;
    for _ in 0..2 {
        let mut host = Host {
            holdings: Vec::new(),
            fault: String::new(),
        };
        let extracted = extract_admitted(&html_path, 4096, &mut host);
        let finished = finish_all(&mut host);
        let value = combine(
            extracted.map_err(|error| format!("{error:#}")),
            finished,
            &host,
        )?;
        html_status = value.status;
        html_contains = value.text.contains("Hello packaging");
        if html_status != "extracted" || !html_contains {
            return Err(format!(
                "html extraction was not the owned text: {html_status}"
            ));
        }
    }
    let epub_path = directory.join("note.epub");
    write_epub(&epub_path)?;
    let mut host = Host {
        holdings: Vec::new(),
        fault: String::new(),
    };
    let extracted = read_epub_admitted(&epub_path, 65_536, &mut host);
    let finished = finish_all(&mut host);
    let epub = combine(
        extracted.map_err(|error| format!("{error:#}")),
        finished,
        &host,
    )?;
    let epub_contains = epub.contains("Packaging Note") && epub.contains("Owned chapter");
    if !epub_contains {
        return Err("epub extraction did not contain the owned title and chapter".to_string());
    }
    let holdings_after = account
        .status()
        .map_err(|error| error.to_string())?
        .holdings;
    if holdings_after != base_holdings {
        return Err(format!(
            "account holdings changed from {base_holdings} to {holdings_after}"
        ));
    }
    Ok(AdmittedText {
        html_status,
        html_contains,
        epub_contains,
        holdings_after,
    })
}

fn combine<T>(
    value: Result<T, String>,
    finished: Result<(), String>,
    host: &Host,
) -> Result<T, String> {
    let fault = if host.fault.is_empty() {
        None
    } else {
        Some(host.fault.clone())
    };
    match (value, finished, fault) {
        (Ok(value), Ok(()), None) => Ok(value),
        (Err(error), Ok(()), None) => Err(error),
        (Ok(_), Err(error), _) => Err(error),
        (Err(error), Err(finish), _) => Err(format!("{error}; holdings: {finish}")),
        (_, Ok(()), Some(fault)) => Err(fault),
    }
}

fn finish_all(host: &mut Host) -> Result<(), String> {
    let mut first = None;
    for holding in &mut host.holdings {
        if let Err(error) = holding.finish() {
            if first.is_none() {
                first = Some(error.to_string());
            }
        }
    }
    match first {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn write_epub(path: &Path) -> Result<(), String> {
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipWriter::new(file);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    write_member(&mut archive, stored, "mimetype", b"application/epub+zip")?;
    write_member(
        &mut archive,
        stored,
        "META-INF/container.xml",
        br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
</container>"#,
    )?;
    write_member(
        &mut archive,
        stored,
        "OEBPS/content.opf",
        br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">
<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Packaging Note</dc:title><dc:identifier id="id">note</dc:identifier></metadata>
<manifest><item id="c1" href="chap.xhtml" media-type="application/xhtml+xml"/></manifest>
<spine><itemref idref="c1"/></spine>
</package>"#,
    )?;
    write_member(
        &mut archive,
        stored,
        "OEBPS/chap.xhtml",
        br#"<?xml version="1.0"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head><title>chapter</title></head><body><p>Owned chapter</p></body></html>"#,
    )?;
    archive.finish().map_err(|error| error.to_string())?;
    Ok(())
}

fn write_member(
    archive: &mut zip::ZipWriter<File>,
    options: zip::write::SimpleFileOptions,
    name: &str,
    bytes: &[u8],
) -> Result<(), String> {
    archive
        .start_file(name, options)
        .map_err(|error| error.to_string())?;
    archive
        .write_all(bytes)
        .map_err(|error| error.to_string())?;
    Ok(())
}
