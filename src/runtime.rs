//! Dependency discovery and explicit Homebrew installation handoff to a visible Terminal window.
//! Terminal owns installation output, input, and lifetime; quitting this app does not cancel it.

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Official prerequisite instructions; the app never runs the Homebrew bootstrap script.
pub const HOMEBREW_INSTALL_URL: &str = "https://brew.sh/";
/// Maximum wait for Launch Services to accept the Terminal request, not for installation to finish.
pub const TERMINAL_LAUNCH_TIMEOUT_SECONDS: u64 = 10;
/// Fixed Homebrew locations accepted for an installation action.
const BREW_PATHS: [&str; 2] = ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"];
/// Standard tool directories checked before inherited absolute PATH entries.
const TOOL_DIRECTORIES: [&str; 2] = ["/opt/homebrew/bin", "/usr/local/bin"];
/// Deterministic Homebrew and macOS utility PATH used only by the generated installer helper.
const INSTALL_PATH: &str = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";
/// Maximum temporary diagnostic output from the launcher, not from the Terminal-owned installer.
const MAX_LAUNCH_DIAGNOSTIC_BYTES: u64 = 64 * 1024;

/// Packages that a clearly labelled user action may install through Homebrew.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrewPackage {
    /// External FFmpeg conversion and ffprobe inspection tools.
    #[serde(rename = "ffmpeg")]
    FFmpeg,
    /// whisper.cpp recognizer; the speech model and library configuration remain separate steps.
    #[serde(rename = "whisper_cpp")]
    WhisperCpp,
}

impl BrewPackage {
    /// Returns a fixed current formula name; arbitrary formula text is unsupported.
    pub fn formula(self) -> &'static str {
        match self {
            Self::FFmpeg => "ffmpeg",
            Self::WhisperCpp => "whisper.cpp",
        }
    }
}

/// Executables used by the media pipeline and dependency guide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeTool {
    /// FFmpeg conversion.
    FFmpeg,
    /// ffprobe stream inspection.
    FFprobe,
    /// whisper.cpp recognition.
    WhisperCli,
}

impl RuntimeTool {
    /// Returns the exact executable basename for a typed tool.
    pub fn executable(self) -> &'static str {
        match self {
            Self::FFmpeg => "ffmpeg",
            Self::FFprobe => "ffprobe",
            Self::WhisperCli => "whisper-cli",
        }
    }
}

/// Availability observed through executable metadata without running tools or loading models.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatus {
    /// Standard-prefix Homebrew, or `None` when prerequisite setup is needed.
    pub homebrew: Option<PathBuf>,
    /// Independently discovered FFmpeg executable.
    pub ffmpeg: Option<PathBuf>,
    /// Independently discovered ffprobe executable.
    pub ffprobe: Option<PathBuf>,
    /// Recognizer availability; a model and ASR configuration are not implied.
    pub whisper_cli: Option<PathBuf>,
}

/// An accepted Terminal launch request, which does not prove installation started or succeeded.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallLaunch {
    /// Allowlisted package selected by the user.
    pub package: BrewPackage,
    /// Fixed standard-prefix Homebrew executable written into the helper.
    pub brew_path: PathBuf,
    /// Private helper retained for Terminal and removed by the helper on normal exit.
    pub command_path: PathBuf,
}

/// Reads standard locations and inherited PATH without executing or installing anything.
/// This checks executable metadata, not codec support, provider health, speech models, or configuration.
pub fn detect_runtime() -> RuntimeStatus {
    RuntimeStatus {
        homebrew: discover_homebrew(),
        ffmpeg: resolve_executable(RuntimeTool::FFmpeg),
        ffprobe: resolve_executable(RuntimeTool::FFprobe),
        whisper_cli: resolve_executable(RuntimeTool::WhisperCli),
    }
}

/// Finds a typed tool at standard Homebrew prefixes before inherited absolute PATH directories.
/// Relative PATH entries are excluded so a knowledge folder cannot inject a tool through its working directory.
pub fn resolve_executable(tool: RuntimeTool) -> Option<PathBuf> {
    let directories: Vec<_> = TOOL_DIRECTORIES.iter().map(PathBuf::from).collect();
    let path = std::env::var_os("PATH");
    resolve_from(tool, &directories, path.as_deref())
}

/// Hands one explicitly requested, allowlisted Homebrew installation to a visible Terminal window.
/// Returns only after the launcher accepts the request; the user reads the result in Terminal and refreshes this app.
/// The helper receives no library content and downloads no speech model or changes library configuration.
/// Installation intentionally continues separately if this app quits; no installer cancellation is claimed.
/// Returns an error for missing Homebrew, helper creation failure, launcher failure, or an unconfirmed launch deadline.
/// Once the launcher starts, errors preserve the helper because Terminal might still execute it.
pub fn launch_install(package: BrewPackage) -> Result<InstallLaunch> {
    let brew = discover_homebrew();
    launch_with(
        brew.as_deref(),
        package,
        Path::new("/usr/bin/open"),
        &std::env::temp_dir(),
        Duration::from_secs(TERMINAL_LAUNCH_TIMEOUT_SECONDS),
    )
}

/// Checks only fixed macOS Homebrew locations, following ordinary Homebrew symlinks.
fn discover_homebrew() -> Option<PathBuf> {
    BREW_PATHS
        .iter()
        .map(PathBuf::from)
        .find(|path| is_executable(path))
}

/// Searches fixed directories followed by absolute inherited PATH entries in deterministic order.
fn resolve_from(
    tool: RuntimeTool,
    directories: &[PathBuf],
    path: Option<&OsStr>,
) -> Option<PathBuf> {
    directories
        .iter()
        .cloned()
        .chain(path.into_iter().flat_map(std::env::split_paths))
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(tool.executable()))
        .find(|candidate| is_executable(candidate))
}

/// Checks a regular executable through metadata; it does not run the candidate.
fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Prepares and dispatches the handoff, with private injectable tools for disposable regressions only.
fn launch_with(
    brew: Option<&Path>,
    package: BrewPackage,
    launcher: &Path,
    temporary_root: &Path,
    timeout: Duration,
) -> Result<InstallLaunch> {
    let brew = brew.context("Homebrew is missing; complete the prerequisite instructions at https://brew.sh/ and check again")?;
    ensure!(
        brew.is_absolute() && is_executable(brew),
        "Homebrew executable is unavailable"
    );
    ensure!(
        !timeout.is_zero(),
        "Terminal launch deadline must be positive"
    );
    let mut diagnostic =
        tempfile::tempfile().context("Cannot allocate Terminal launch diagnostics")?;
    let (directory, command_path) = prepare_helper(temporary_root, brew, package)?;
    let mut command = Command::new(launcher);
    command
        .args(["-a", "Terminal"])
        .arg(&command_path)
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            diagnostic
                .try_clone()
                .context("Cannot prepare Terminal launch diagnostics")?,
        );
    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let cleanup = directory.close();
            return Err(match cleanup {
                Ok(()) => anyhow!(
                    "Cannot start Terminal launcher: {error}; the unused helper was removed"
                ),
                Err(cleanup) => anyhow!(
                    "Cannot start Terminal launcher: {error}; unused helper cleanup failed at {}: {cleanup}",
                    command_path.display()
                ),
            });
        }
    };
    // A launched request can reach Terminal even if open later fails or times out; never delete its helper here.
    let _retained_directory = directory.keep();
    wait_launcher(child, &mut diagnostic, timeout).map_err(|error| {
        anyhow!("Terminal launch request was not confirmed: {error:#}. Terminal may have started; helper retained at {}", command_path.display())
    })?;
    Ok(InstallLaunch {
        package,
        brew_path: brew.into(),
        command_path,
    })
}

/// Creates a unique private directory and an executable helper containing only fixed, allowlisted commands.
/// The caller retains the directory before returning from an accepted or uncertain launch.
fn prepare_helper(root: &Path, brew: &Path, package: BrewPackage) -> Result<(TempDir, PathBuf)> {
    let directory = tempfile::Builder::new()
        .prefix("omni-knowledge-install-")
        .tempdir_in(root)
        .context("Cannot create private installation helper directory")?;
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))?;
    let command_path = directory.path().join("install.command");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&command_path)
        .context("Cannot create private installation helper")?;
    file.set_permissions(fs::Permissions::from_mode(0o700))?;
    let brew = shell_quote(brew.to_str().context("Homebrew path must be UTF-8")?);
    let script = shell_quote(
        command_path
            .to_str()
            .context("Installer helper path must be UTF-8")?,
    );
    let folder = shell_quote(
        directory
            .path()
            .to_str()
            .context("Installer directory must be UTF-8")?,
    );
    let formula = package.formula();
    let text = format!(
        r#"#!/bin/sh
set -u
export PATH='{INSTALL_PATH}'
export HOMEBREW_NO_ANALYTICS=1
export HOMEBREW_NO_AUTO_UPDATE=1
export HOMEBREW_NO_INSTALL_CLEANUP=1
cd /
omni_cleanup() {{
    /bin/rm -f {script}
    /bin/rmdir {folder} 2>/dev/null || true
}}
trap omni_cleanup EXIT
printf 'Omni Knowledge: install {formula} through Homebrew.\n'
printf 'This visible Terminal installation continues if Omni Knowledge quits.\n\n'
{brew} install '{formula}'
omni_install_exit=$?
printf '\nHomebrew exit status: %s\n' "$omni_install_exit"
if [ "$omni_install_exit" -eq 0 ]; then
    printf 'Homebrew completed. Return to Omni Knowledge and select Refresh dependencies.\n'
else
    printf 'Homebrew did not complete. Read the error above; partial package state may remain.\n'
    printf 'Check Homebrew before retrying, then refresh dependencies in Omni Knowledge.\n'
fi
printf '\nPress Return to close this installer.\n'
IFS= read -r omni_install_ack || true
exit "$omni_install_exit"
"#
    );
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok((directory, command_path))
}

/// Quotes trusted fixed paths and app-owned temporary paths as one POSIX shell word.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Waits only for the launch request and bounds its diagnostics; it never waits for or kills Terminal or Homebrew.
fn wait_launcher(child: Child, diagnostic: &mut File, timeout: Duration) -> Result<()> {
    let mut process = LauncherProcess {
        child,
        finished: false,
    };
    let started = Instant::now();
    loop {
        ensure!(
            diagnostic.metadata()?.len() <= MAX_LAUNCH_DIAGNOSTIC_BYTES,
            "Terminal launcher diagnostics exceed 64 KiB"
        );
        if let Some(status) = process
            .child
            .try_wait()
            .context("Cannot observe Terminal launcher")?
        {
            process.finished = true;
            if status.success() {
                return Ok(());
            }
            diagnostic.seek(SeekFrom::Start(0))?;
            let mut bytes = Vec::new();
            diagnostic.take(4096).read_to_end(&mut bytes)?;
            bail!(
                "launcher exited with {status}: {}",
                String::from_utf8_lossy(&bytes).trim()
            );
        }
        ensure!(
            started.elapsed() < timeout,
            "Terminal launch deadline expired; outcome is unknown"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

/// Owns only the short-lived launcher process, never the intentionally separate Terminal installation.
struct LauncherProcess {
    /// Direct open request process to reap on deadline or I/O failure.
    child: Child,
    /// Whether the process has already exited and was reaped.
    finished: bool,
}

impl Drop for LauncherProcess {
    /// Stops and reaps only an unfinished launcher request while preserving the helper for a possible handoff.
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Owns fake local executables and a separate disposable helper directory.
    struct Fixture {
        directory: TempDir,
        brew: PathBuf,
        launcher: PathBuf,
        requests: PathBuf,
    }

    impl Fixture {
        /// Creates mock tools without exposing writable executable descriptors to parallel forks.
        /// No real Terminal or Homebrew installation is invoked; setup failures fail the test.
        fn new(brew_script: &str, launcher_script: &str) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let requests = directory.path().join("requests");
            fs::create_dir(&requests).unwrap();
            let brew = directory.path().join("brew 'quoted path");
            let launcher = directory.path().join("open");
            // Only this child opens scripts for writing; sibling forks cannot inherit those FDs.
            // See docs/changes/2026-11-04-test-disposition.md for the Linux ETXTBSY evidence.
            let status = Command::new("/bin/sh")
                .arg("-c")
                .arg("printf '#!/bin/sh\\n%s\\n' \"$2\" > \"$1\" && printf '#!/bin/sh\\n%s\\n' \"$4\" > \"$3\"")
                .arg("fixture-writer")
                .arg(&brew)
                .arg(brew_script)
                .arg(&launcher)
                .arg(launcher_script)
                .status()
                .unwrap();
            assert!(
                status.success(),
                "synthetic executable setup failed: {status}"
            );
            for path in [&brew, &launcher] {
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
            Self {
                directory,
                brew,
                launcher,
                requests,
            }
        }

        /// Lists only helpers created beneath this fixture's separate requests directory.
        fn helpers(&self) -> Vec<PathBuf> {
            fs::read_dir(&self.requests)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect()
        }

        /// Makes one short-lived request through the fake launcher.
        fn launch(&self, package: BrewPackage) -> Result<InstallLaunch> {
            launch_with(
                Some(&self.brew),
                package,
                &self.launcher,
                &self.requests,
                Duration::from_secs(2),
            )
        }
    }

    /// Validates fixed arguments, private modes, and an accepted launch without executing the installer.
    #[test]
    fn accepted_launch_preserves_private_helper_without_claiming_installation() {
        let fixture = Fixture::new(
            "printf unexpected > \"${0}.ran\"",
            "printf '%s\\n' \"$@\" > \"${0}.arguments\"",
        );
        let launch = fixture.launch(BrewPackage::FFmpeg).unwrap();
        assert_eq!(launch.package, BrewPackage::FFmpeg);
        assert_eq!(launch.brew_path, fixture.brew);
        assert!(launch.command_path.is_file());
        assert_eq!(
            launch.command_path.metadata().unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            launch
                .command_path
                .parent()
                .unwrap()
                .metadata()
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let arguments = fs::read_to_string(fixture.launcher.with_extension("arguments")).unwrap();
        assert_eq!(
            arguments.lines().collect::<Vec<_>>(),
            vec!["-a", "Terminal", launch.command_path.to_str().unwrap()]
        );
        assert!(!fixture.brew.with_extension("ran").exists());
        let script = fs::read_to_string(&launch.command_path).unwrap();
        assert!(script.contains(&format!("export PATH='{INSTALL_PATH}'")));
        assert!(script.contains("continues if Omni Knowledge quits"));
        assert!(!script.contains("sudo"));
        assert!(!script.contains("curl"));
        assert!(fixture.directory.path().is_dir());
    }

    /// Executes only a disposable fake brew through the generated helper and checks result output plus self-cleanup.
    #[test]
    fn generated_helper_dispatches_typed_formula_and_removes_only_itself() {
        let fixture = Fixture::new(
            "printf 'argc=%s\\n' \"$#\"\nprintf 'arg=%s\\n' \"$@\"",
            "exit 0",
        );
        for package in [BrewPackage::FFmpeg, BrewPackage::WhisperCpp] {
            let launch = fixture.launch(package).unwrap();
            let output = Command::new(&launch.command_path)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert!(output.status.success());
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains("argc=2\narg=install\n"));
            assert!(text.contains(&format!("arg={}\n", package.formula())));
            assert!(text.contains("Homebrew exit status: 0"));
            assert!(text.contains("Refresh dependencies"));
            assert!(!launch.command_path.exists());
            assert!(!launch.command_path.parent().unwrap().exists());
            assert!(fixture.brew.exists());
            assert!(fixture.launcher.exists());
        }
    }

    /// Shows an installation error in the separate script output while preserving exit status and removing its helper.
    #[test]
    fn helper_failure_reports_partial_state_in_terminal_output() {
        let fixture = Fixture::new("printf 'mock package rejected\\n' >&2\nexit 7", "exit 0");
        let launch = fixture.launch(BrewPackage::WhisperCpp).unwrap();
        let output = Command::new(&launch.command_path)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("partial package state may remain")
        );
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("mock package rejected")
        );
        assert!(!launch.command_path.parent().unwrap().exists());
    }

    /// Missing Homebrew must produce a prerequisite error before creating or launching a helper.
    #[test]
    fn absent_homebrew_creates_no_helper() {
        let fixture = Fixture::new("exit 0", "printf launched > \"${0}.ran\"");
        let error = launch_with(
            None,
            BrewPackage::FFmpeg,
            &fixture.launcher,
            &fixture.requests,
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error.to_string().contains("https://brew.sh/"));
        assert!(fixture.helpers().is_empty());
        assert!(!fixture.launcher.with_extension("ran").exists());
    }

    /// A definite failure to spawn the launcher removes the unused helper and leaves fixture tools intact.
    #[test]
    fn launcher_spawn_failure_cleans_up_unused_helper() {
        let fixture = Fixture::new("exit 0", "exit 0");
        let missing = fixture.directory.path().join("missing-launcher");
        let error = launch_with(
            Some(&fixture.brew),
            BrewPackage::FFmpeg,
            &missing,
            &fixture.requests,
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error.to_string().contains("unused helper was removed"));
        assert!(fixture.helpers().is_empty());
        assert!(fixture.brew.exists());
    }

    /// A launched request with an error preserves its helper because another application might still receive it.
    #[test]
    fn nonzero_launcher_exit_reports_uncertainty_and_preserves_helper() {
        let fixture = Fixture::new("exit 0", "printf 'mock launcher rejected\\n' >&2\nexit 9");
        let error = fixture.launch(BrewPackage::FFmpeg).unwrap_err().to_string();
        assert!(error.contains("exit status: 9"), "{error}");
        assert!(error.contains("mock launcher rejected"));
        assert!(error.contains("Terminal may have started"));
        assert_eq!(fixture.helpers().len(), 1);
        assert!(fixture.helpers()[0].join("install.command").is_file());
    }

    /// A deadline stops only the mock launcher and keeps a helper that Terminal might still read.
    #[test]
    fn launcher_timeout_preserves_helper_without_claiming_handoff() {
        let fixture = Fixture::new("exit 0", "exec /bin/sleep 5");
        let started = Instant::now();
        let error = launch_with(
            Some(&fixture.brew),
            BrewPackage::FFmpeg,
            &fixture.launcher,
            &fixture.requests,
            Duration::from_millis(400),
        )
        .unwrap_err()
        .to_string();
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(error.contains("deadline expired"));
        assert!(error.contains("outcome is unknown"));
        assert!(error.contains("helper retained"));
        assert_eq!(fixture.helpers().len(), 1);
    }

    /// Resolves fixed prefixes independently with a limited PATH and rejects working-directory PATH entries.
    /// Synthetic PATH directories make absence independent of installed media prerequisites.
    #[test]
    fn fixed_tool_directories_work_with_limited_path() {
        let fixed = tempfile::tempdir().unwrap();
        let fallback = tempfile::tempdir().unwrap();
        let absent = tempfile::tempdir().unwrap();
        let absent_path = std::env::join_paths([absent.path()]).unwrap();
        for path in [
            fixed.path().join("ffmpeg"),
            fallback.path().join("ffmpeg"),
            fallback.path().join("ffprobe"),
        ] {
            fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let directories = vec![fixed.path().into()];
        let path = std::env::join_paths([fallback.path()]).unwrap();
        assert_eq!(
            resolve_from(RuntimeTool::FFmpeg, &directories, Some(&path)),
            Some(fixed.path().join("ffmpeg"))
        );
        assert_eq!(
            resolve_from(RuntimeTool::FFprobe, &directories, Some(&path)),
            Some(fallback.path().join("ffprobe"))
        );
        assert!(resolve_from(RuntimeTool::WhisperCli, &directories, Some(&path)).is_none());
        assert!(resolve_from(RuntimeTool::FFprobe, &directories, Some(&absent_path)).is_none());
        assert!(resolve_from(RuntimeTool::FFmpeg, &[], Some(OsStr::new(".:relative"))).is_none());
    }
}
