//! Observed WebView bridge traffic and the other spike probes.
//!
//! The session writes facts. It does not decide the pass/fail matrix.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use omni_shell_fake::Store;
use serde_json::{Value, json};

/// Which automatic exit the process was asked to use.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Stay open. No evidence deadline.
    Serve,
    /// Record bridge traffic and the page's DOM report.
    Probe,
    /// Wait for a WebView composition event.
    Ime,
    /// Wait for a native file drop.
    Drop,
    /// Wait for the file-dialog answer.
    Dialog,
}

/// One spike process.
pub struct Session {
    store: Store,
    mode: Mode,
    evidence_path: Option<PathBuf>,
    port: u16,
    started: Instant,
    started_unix_ms: u128,
    pid: u32,
    nonce: String,
    ping_sent: bool,
    calls: Vec<String>,
    ack_ops: Vec<String>,
    bridge_ready: bool,
    dom: Option<Value>,
    ime: Vec<Value>,
    drops: Vec<Vec<String>>,
    dialog: Option<String>,
    sidebar_presses: u32,
    menu_installed: bool,
    menu_selects: u32,
    native_ime: Vec<String>,
    native_composition: Vec<Value>,
    written: bool,
}

impl Session {
    /// Start the fixture store for `port` and read the process mode.
    pub fn start(port: u16) -> Rc<RefCell<Self>> {
        let mode = match std::env::var("OMNI_SHELL_SPIKE_MODE").as_deref() {
            Ok("probe") => Mode::Probe,
            Ok("ime") => Mode::Ime,
            Ok("drop") => Mode::Drop,
            Ok("dialog") => Mode::Dialog,
            _ => Mode::Serve,
        };
        let evidence_path = std::env::var("OMNI_SHELL_SPIKE_EVIDENCE")
            .ok()
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let nonce = format!("ping-{}", unix_ms());
        Rc::new(RefCell::new(Self {
            store: Store::new(format!("http://127.0.0.1:{port}")),
            mode,
            evidence_path,
            port,
            started: Instant::now(),
            started_unix_ms: unix_ms(),
            pid: std::process::id(),
            nonce,
            ping_sent: false,
            calls: Vec::new(),
            ack_ops: Vec::new(),
            bridge_ready: false,
            dom: None,
            ime: Vec::new(),
            drops: Vec::new(),
            dialog: None,
            sidebar_presses: 0,
            menu_installed: false,
            menu_selects: 0,
            native_ime: Vec::new(),
            native_composition: Vec::new(),
            written: false,
        }))
    }

    /// Root folders for the native sidebar.
    pub fn mounts(&self) -> Vec<(String, String)> {
        self.store.mounts()
    }

    /// The nonce of the unsolicited host ping.
    pub fn nonce(&self) -> &str {
        &self.nonce
    }

    /// Record that `set_application_menu` ran.
    pub fn menu_installed(&mut self) {
        self.menu_installed = true;
    }

    /// Record that a menu item's select handler ran.
    pub fn menu_selected(&mut self) {
        self.menu_selects += 1;
    }

    /// Record a native sidebar press.
    pub fn sidebar_pressed(&mut self) {
        self.sidebar_presses += 1;
    }

    /// Record text from the native input control.
    pub fn native_ime(&mut self, text: &str) {
        self.native_ime.push(text.to_owned());
    }

    /// Record a native input-method composition event.
    pub fn native_composition(&mut self, composing: bool, text: &str) {
        self.native_composition.push(json!({
            "composing": composing,
            "text": text,
        }));
    }

    /// Record file URLs delivered by the platform drop handler.
    pub fn file_drop(&mut self, uris: &[String]) {
        self.drops.push(uris.to_vec());
    }

    /// Record the file dialog's raw result.
    pub fn dialog_result(&mut self, result: &str) {
        self.dialog = Some(result.to_owned());
    }

    /// Handle one page message and return texts to post back.
    pub fn on_page(&mut self, raw: &str) -> Vec<String> {
        let Ok(message) = serde_json::from_str::<Value>(raw) else {
            self.calls.push("invalid-json".to_owned());
            return Vec::new();
        };
        match message.get("kind").and_then(Value::as_str) {
            Some("call") => {
                let op = message
                    .get("op")
                    .and_then(Value::as_str)
                    .unwrap_or("missing")
                    .to_owned();
                self.calls.push(op);
                let reply = self.store.reply_to(&message);
                let mut out = vec![reply.to_string()];
                if !self.ping_sent {
                    self.ping_sent = true;
                    out.push(
                        json!({
                            "kind": "host",
                            "op": "ping",
                            "nonce": self.nonce,
                        })
                        .to_string(),
                    );
                }
                out
            }
            Some("ack") => {
                let op = message
                    .get("echo")
                    .and_then(|echo| echo.get("op"))
                    .and_then(Value::as_str)
                    .unwrap_or("ack")
                    .to_owned();
                self.ack_ops.push(op);
                Vec::new()
            }
            Some("probe") => {
                match message.get("event").and_then(Value::as_str) {
                    Some("bridge-ready") => self.bridge_ready = true,
                    Some("ime") => self.ime.push(message),
                    Some("report") => self.dom = message.get("dom").cloned(),
                    _ => {}
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    /// Whether this mode should write evidence and close.
    pub fn due(&self) -> bool {
        let elapsed = self.started.elapsed();
        match self.mode {
            Mode::Serve => false,
            Mode::Probe => {
                (self.dom.is_some() && elapsed >= Duration::from_secs(8))
                    || elapsed >= Duration::from_secs(45)
            }
            Mode::Ime => !self.ime.is_empty() || elapsed >= Duration::from_secs(40),
            Mode::Drop => !self.drops.is_empty() || elapsed >= Duration::from_secs(20),
            Mode::Dialog => self.dialog.is_some() || elapsed >= Duration::from_secs(12),
        }
    }

    /// The active mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Write the observation once.
    pub fn write(&mut self) {
        if self.written {
            return;
        }
        self.written = true;
        let body = json!({
            "backend": {
                "mode": "fake-knowledge-client",
                "omniServe": "not-used",
                "reason": "The current service is preparatory. This process does not listen under that name."
            },
            "nshCrates": "0.4.0",
            "pid": self.pid,
            "port": self.port,
            "startedUnixMs": self.started_unix_ms,
            "elapsedMs": self.started.elapsed().as_millis(),
            "mode": mode_name(self.mode),
            "bridgeReady": self.bridge_ready,
            "calls": self.calls,
            "pingNonce": self.nonce,
            "pingSent": self.ping_sent,
            "ackOps": self.ack_ops,
            "dom": self.dom,
            "webviewIme": self.ime,
            "nativeImeText": self.native_ime,
            "nativeComposition": self.native_composition,
            "drops": self.drops,
            "dialog": self.dialog,
            "sidebarPresses": self.sidebar_presses,
            "menuInstalled": self.menu_installed,
            "menuSelects": self.menu_selects,
        });
        let text = serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_owned());
        if let Some(path) = &self.evidence_path {
            if let Some(parent) = path.parent()
                && !parent.as_os_str().is_empty()
            {
                let _ = fs::create_dir_all(parent);
            }
            match fs::write(path, text.as_bytes()) {
                Ok(()) => eprintln!("spike-evidence {}", path.display()),
                Err(error) => eprintln!("spike-evidence-error {error}"),
            }
        } else {
            eprintln!("spike-evidence-inline {text}");
        }
    }
}

fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Serve => "serve",
        Mode::Probe => "probe",
        Mode::Ime => "ime",
        Mode::Drop => "drop",
        Mode::Dialog => "dialog",
    }
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
