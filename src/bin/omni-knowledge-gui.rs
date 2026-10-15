//! Start the native GPUI knowledge library window.

/// Open an optional knowledge root from the first command-line argument.
fn main() -> anyhow::Result<()> {
    let root = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    omni_knowledge::gui::run(root)
}
