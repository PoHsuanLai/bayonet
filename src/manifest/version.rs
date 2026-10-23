//! A plugin's version.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// A dotted version such as `1.2.0`, compared number by number: `1.10` is newer than `1.9`, and
/// trailing zeros do not matter (`1.2` equals `1.2.0`). A semver pre-release such as
/// `0.1.0-beta.1` ranks below its release, and pre-releases compare as semver does. It prints as
/// it was written.
#[derive(Debug, Clone)]
pub struct Version {
    numbers: Vec<u32>,
    pre: Vec<PreRelease>,
}

/// One dot-separated part of a pre-release: digits compare as numbers and rank below words.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum PreRelease {
    Number(u64),
    Word(String),
}

/// Text that is not a dotted version.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a version: {version:?}")]
pub struct InvalidVersion {
    /// The text as written.
    pub version: String,
}

impl Version {
    /// `text` as a version: numbers separated by single dots, such as `2` or `1.4.12`, then
    /// optionally `-` and a pre-release of dot-separated letters, digits and hyphens, such as
    /// `1.4.12-beta.1`.
    pub fn parse(text: &str) -> Result<Version, InvalidVersion> {
        let invalid = || InvalidVersion {
            version: text.to_owned(),
        };
        let (numbers, pre) = match text.split_once('-') {
            Some((numbers, pre)) => (numbers, Some(pre)),
            None => (text, None),
        };
        let numbers: Vec<u32> = numbers
            .split('.')
            .map(|part| digits(part).then(|| part.parse().ok()).flatten())
            .collect::<Option<_>>()
            .ok_or_else(invalid)?;
        let pre: Vec<PreRelease> = pre
            .map(|pre| pre.split('.').map(PreRelease::parse).collect::<Option<_>>())
            .unwrap_or(Some(Vec::new()))
            .ok_or_else(invalid)?;
        Ok(Version { numbers, pre })
    }

    /// The numbers, as written.
    pub fn parts(&self) -> &[u32] {
        &self.numbers
    }

    /// The numbers without the zeros that end them: what comparing and hashing look at.
    fn significant(&self) -> &[u32] {
        let end = self
            .numbers
            .iter()
            .rposition(|&n| n != 0)
            .map_or(0, |at| at + 1);
        &self.numbers[..end]
    }
}

/// Whether `part` is one or more ASCII digits.
fn digits(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())
}

impl PreRelease {
    /// One pre-release part: digits without a leading zero, or letters, digits and hyphens.
    fn parse(part: &str) -> Option<PreRelease> {
        if digits(part) {
            let leading_zero = part.len() > 1 && part.starts_with('0');
            return (!leading_zero)
                .then(|| part.parse().ok().map(PreRelease::Number))
                .flatten();
        }
        let word = !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        word.then(|| PreRelease::Word(part.to_owned()))
    }
}

impl fmt::Display for PreRelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreRelease::Number(n) => write!(f, "{n}"),
            PreRelease::Word(word) => f.write_str(word),
        }
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Version) -> bool {
        self.significant() == other.significant() && self.pre == other.pre
    }
}

impl Eq for Version {}

impl Hash for Version {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.significant().hash(state);
        self.pre.hash(state);
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        let pre = match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => self.pre.cmp(&other.pre),
        };
        self.significant().cmp(other.significant()).then(pre)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let numbers: Vec<String> = self.numbers.iter().map(u32::to_string).collect();
        f.write_str(&numbers.join("."))?;
        if !self.pre.is_empty() {
            let pre: Vec<String> = self.pre.iter().map(ToString::to_string).collect();
            write!(f, "-{}", pre.join("."))?;
        }
        Ok(())
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
            ("a pre-release before its release", "0.1.0-beta.1", "0.1.0"),
            (
                "a pre-release after the release before",
                "0.0.9",
                "0.1.0-alpha",
            ),
            (
                "numbered pre-releases by number",
                "1.0.0-beta.2",
                "1.0.0-beta.11",
            ),
            ("digits before words", "1.0.0-1", "1.0.0-alpha"),
            (
                "a longer pre-release after its prefix",
                "1.0.0-alpha",
                "1.0.0-alpha.1",
            ),
            ("words by their letters", "1.0.0-alpha.1", "1.0.0-beta"),
        ];
        for (name, older, newer) in cases {
            assert!(version(older) < version(newer), "{name}");
        }
    }

    #[test]
    fn trailing_zeros_do_not_make_versions_differ_but_still_print_as_written() {
        assert_eq!(version("1.2"), version("1.2.0"));
        assert_eq!(version("1.2.0").to_string(), "1.2.0");
        assert_eq!(version("0.1-beta.1"), version("0.1.0-beta.1"));
        assert_eq!(version("0.1.0-beta.1").to_string(), "0.1.0-beta.1");
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
            "1.2-",
            "1.2-beta.",
            "1.2-beta..1",
            "1.2-beta.01",
            "1.2-be_ta",
            "1.2+build",
            "-1",
            "99999999999",
        ] {
            assert!(Version::parse(text).is_err(), "{text:?}");
        }
    }
}
