//! Read-only public regressions for dependency discovery and typed installation choices.

use omni_knowledge::runtime::{
    BrewPackage, HOMEBREW_INSTALL_URL, RuntimeStatus, RuntimeTool, detect_runtime,
    resolve_executable,
};
use std::path::Path;

/// Reads only executable metadata and checks that every advertised path actually exists.
#[test]
fn actual_detection_reports_existing_executables() {
    let status = detect_runtime();
    for executable in [
        &status.homebrew,
        &status.ffmpeg,
        &status.ffprobe,
        &status.whisper_cli,
    ]
    .into_iter()
    .flatten()
    {
        assert!(executable.is_absolute());
        assert!(executable.is_file());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_ne!(
                executable.metadata().unwrap().permissions().mode() & 0o111,
                0
            );
        }
    }
    if let Some(brew) = &status.homebrew {
        assert!(matches!(
            brew.to_str(),
            Some("/opt/homebrew/bin/brew" | "/usr/local/bin/brew")
        ));
    }
    assert_eq!(status.ffmpeg, resolve_executable(RuntimeTool::FFmpeg));
    assert_eq!(status.ffprobe, resolve_executable(RuntimeTool::FFprobe));
    assert_eq!(
        status.whisper_cli,
        resolve_executable(RuntimeTool::WhisperCli)
    );
}

/// Confirms both installer choices map to fixed current Homebrew formula names.
#[test]
fn package_allowlist_rejects_argument_injection() {
    assert_eq!(BrewPackage::FFmpeg.formula(), "ffmpeg");
    assert_eq!(BrewPackage::WhisperCpp.formula(), "whisper.cpp");
    for rejected in [
        "ffmpeg; touch marker",
        "--formula",
        "curl",
        "whisper.cpp --HEAD",
    ] {
        let encoded = serde_json::to_string(rejected).unwrap();
        assert!(serde_json::from_str::<BrewPackage>(&encoded).is_err());
    }
    assert_eq!(HOMEBREW_INSTALL_URL, "https://brew.sh/");
}

/// Confirms status records preserve explicit absent dependencies for the guide and CLI.
#[test]
fn discovery_status_round_trips_without_installing() {
    let status = RuntimeStatus {
        homebrew: Some(Path::new("/opt/homebrew/bin/brew").into()),
        ffmpeg: None,
        ffprobe: None,
        whisper_cli: None,
    };
    let json = serde_json::to_string(&status).unwrap();
    let restored: RuntimeStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, status);
}
