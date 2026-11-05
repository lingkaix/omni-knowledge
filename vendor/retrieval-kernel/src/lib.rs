//! The domain-free Retrieval Kernel. Canonical files remain with their adapters.
pub mod digest_window;
pub mod directory;
mod model;
mod query;
mod store;
mod text;
pub use directory::{ByteDirectory, SourceGeneration};
pub use model::*;
pub use store::{GenerationFormat, GenerationPin, Kernel, consumer_machinery, generation_format};
pub use text::{Control, analyze, tokens};

#[cfg(test)]
mod tests;
