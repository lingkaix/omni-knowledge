//! Execute the downstream probe without a test-only kernel entry point.

/// Run the real consumer; return a nonzero process result on an error or assertion failure.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", kernel_probe::run_probe()?);
    Ok(())
}
