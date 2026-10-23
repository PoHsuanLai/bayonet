//! A plugin's version.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A dotted version such as `1.2.0`, compared number by number: `1.10` is newer than `1.9`, and
/// trailing zeros do not matter (`1.2` equals `1.2.0`). It prints as it was written.
#[derive(Debug, Clone)]
pub struct Version(Vec<u32>);

/// Text that is not a dotted version.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a version: {version:?}")]
pub struct InvalidVersion {
    /// The text as written.
    pub version: String,
}

impl Version {
    /// `text` as a version: numbers separated by single dots, such as `2` or `1.4.12`.
    pub fn parse(text: &str) -> Result<Version, InvalidVersion> {
        let parts: Option<Vec<u32>> = text
            .split('.')
            .map(|part| {
                let digits = !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
                digits.then(|| part.parse().ok()).flatten()
            })
            .collect();
        parts.map(Version).ok_or_else(|| InvalidVersion {
            version: text.to_owned(),
        })
    }

    /// The numbers, as written.
    pub fn parts(&self) -> &[u32] {
        &self.0
    }

    /// The numbers without the zeros that end them: what comparing and hashing look at.
    fn significant(&self) -> &[u32] {
        let end = self.0.iter().rposition(|&n| n != 0).map_or(0, |at| at + 1);
        &self.0[..end]
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Version) -> bool {
        self.significant() == other.significant()
    }
}

impl Eq for Version {}

impl Hash for Version {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.significant().hash(state);
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        self.significant().cmp(other.significant())
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text: Vec<String> = self.0.iter().map(u32::to_string).collect();
        f.write_str(&text.join("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    #[test]
    fn versions_compare_number_by_number() {
        // name, the older, the newer
        let cases: &[(&str, &str, &str)] = &[
            ("two digits beat one", "1.9", "1.10"),
            ("a later part decides", "1.2.0", "1.2.1"),
            ("a longer version with a non-zero tail", "1.2", "1.2.1"),
            ("the first part first", "1.99", "2.0"),
        ];
        for (name, older, newer) in cases {
            assert!(version(older) < version(newer), "{name}");
        }
    }

    #[test]
    fn trailing_zeros_do_not_make_versions_differ_but_still_print_as_written() {
        assert_eq!(version("1.2"), version("1.2.0"));
        assert_eq!(version("1.2.0").to_string(), "1.2.0");
    }

    #[test]
    fn text_that_is_not_dotted_numbers_is_not_a_version() {
        for text in [
            "",
            ".",
            "1.",
            ".1",
            "1..2",
            "1.x",
            "v1",
            "1.2-beta",
            "-1",
            "99999999999",
        ] {
            assert!(Version::parse(text).is_err(), "{text:?}");
        }
    }
}
