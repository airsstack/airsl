//! Toy core library shared by the native module.
/// A value the native module exposes, proving the path dependency links.
#[must_use]
pub const fn answer() -> i64 {
    42
}
