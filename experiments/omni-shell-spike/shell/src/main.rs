//! Runs the spike directly on this machine's native backend.
//!
//! `nsh package` builds its own runner and mounts [`omni_shell_spike::Shell`].
//! This binary is the same root for `cargo run`.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() -> impl std::process::Termination {
    env_logger::init();
    platform::run_app(
        platform::AppSource::Rust(platform::Root::of::<omni_shell_spike::Shell>()),
        platform::AppOptions {
            title: "Omni Shell Spike".to_owned(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: "dev.omniknowledge.shell-spike".to_owned(),
            size: (1100, 720),
            ..Default::default()
        },
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn main() {
    compile_error!("Run this binary on macOS, Windows, or Linux.");
}
