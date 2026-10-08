//! Finding manifests on disk.

mod search;

pub use search::{Discovery, Rejected, Search, discover};

#[cfg(test)]
mod tests;
