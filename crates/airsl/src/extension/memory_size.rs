//! The `"64MB"` spelling of a memory ceiling.
//!
//! Its own module because a manifest is written by hand and `memory = 67108864` is not a
//! number a person checks by eye. The grammar is deliberately small: an integer, optionally
//! followed by a binary unit. Both `MB` and `MiB` mean 1024², because a ceiling spelled in
//! decimal megabytes would be 4.8% smaller than the author expected, and nobody wants that.
//!
//! Responsibilities: [`parse_memory_size`].
//!
//! Non-responsibilities: the ceiling itself ([`crate::MemoryLimit`]).

use crate::error::{Error, Result};
use crate::sandbox::MemoryLimit;

/// Field name reported when a size does not parse.
const FIELD: &str = "limits.memory";

/// Parses `"64MB"`, `"64MiB"`, `"512kb"`, `"1GB"` or a bare byte count into a ceiling.
///
/// Units are case-insensitive and attach directly to the number. `K`, `M` and `G` each mean a
/// power of 1024; the `i` in `KiB` is accepted and changes nothing.
///
/// # Errors
///
/// Returns [`Error::ManifestInvalid`] for an empty string, a non-numeric prefix, an unknown
/// unit, a value of zero, or a product that overflows `usize`.
pub fn parse_memory_size(raw: &str) -> Result<MemoryLimit> {
    let invalid = |reason: String| Error::ManifestInvalid {
        field: FIELD,
        reason,
    };

    let trimmed = raw.trim();
    let digits_end = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (digits, unit) = trimmed.split_at(digits_end);
    if digits.is_empty() {
        return Err(invalid(format!("`{raw}` does not start with a number")));
    }
    let count: usize = digits
        .parse()
        .map_err(|_| invalid(format!("`{raw}` is not a valid size")))?;

    let multiplier: usize = match unit.to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        other => return Err(invalid(format!("`{other}` is not a known unit in `{raw}`"))),
    };

    let bytes = count
        .checked_mul(multiplier)
        .ok_or_else(|| invalid(format!("`{raw}` is too large")))?;
    if bytes == 0 {
        return Err(invalid(format!("`{raw}` is zero")));
    }
    Ok(MemoryLimit::bytes(bytes))
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "tests unwrap known-valid fixtures; a panic is the intended failure signal"
    )]

    use super::parse_memory_size;

    #[test]
    fn units_are_binary_and_case_insensitive() {
        for (raw, expected) in [
            ("64MB", 64 * 1024 * 1024),
            ("64mib", 64 * 1024 * 1024),
            ("64M", 64 * 1024 * 1024),
            ("512KB", 512 * 1024),
            ("1gb", 1024 * 1024 * 1024),
            ("4096", 4096),
            ("4096B", 4096),
            (" 8MB ", 8 * 1024 * 1024),
        ] {
            assert_eq!(parse_memory_size(raw).unwrap().get(), expected, "{raw}");
        }
    }

    #[test]
    fn rejects_what_is_not_a_size() {
        for raw in ["", "MB", "64 MB", "64TB", "64.5MB", "-1MB", "0", "0MB"] {
            let err = parse_memory_size(raw).unwrap_err();
            assert!(err.to_string().contains("limits.memory"), "{raw}: {err}");
        }
    }

    #[test]
    fn rejects_overflow() {
        assert!(parse_memory_size(&format!("{}GB", usize::MAX)).is_err());
    }
}
