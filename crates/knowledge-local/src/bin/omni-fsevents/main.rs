//! Finite owned FSEvents helper. Only native macOS execution qualifies the Apple binding.
#[cfg(target_os = "macos")]
mod apple;
#[path = "../../fsevents_protocol.rs"]
#[allow(dead_code)]
mod protocol;

/// Run only the selected platform helper; unsupported hosts never report native startup success.
fn main() {
    #[cfg(target_os = "macos")]
    let code = apple::run();
    #[cfg(not(target_os = "macos"))]
    let code = 78;
    // Process exit, confirmed by the exact parent's wait, is the callback/CF/FD lifetime boundary.
    // Do not destroy a callback context based on a void native stop call or pipe closure.
    std::process::exit(code);
}
