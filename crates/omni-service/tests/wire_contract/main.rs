//! Independent checks for the version-1 service wire codec.
//!
//! Expectations come from the committed service contract and the public crate
//! surface. These tests do not open a library, transport, or user interface.

mod bounds;
mod goldens;
mod integers;
mod regressions;
mod results;
mod support;
