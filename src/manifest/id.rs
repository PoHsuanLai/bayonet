//! A plugin's id.

/// A plugin's id: lower-case letters, digits, `-` and `_`, up to 64 of them. It names the
/// manifest file and decides which manifest overrides which.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PluginId(String);

/// Text that is not a plugin id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("not a plugin id: {id:?}")]
pub struct InvalidId {
    /// The text as written.
    pub id: String,
}

impl PluginId {
    /// `text` as an id, or why it is not one.
    pub fn parse(text: &str) -> Result<PluginId, InvalidId> {
        let fits = !text.is_empty()
            && text.len() <= 64
            && text
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if fits {
            Ok(PluginId(text.to_owned()))
        } else {
            Err(InvalidId {
                id: text.to_owned(),
            })
        }
    }

    /// The id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_lower_case_words_of_at_most_sixty_four_characters() {
        let long = "a".repeat(65);
        let fits = "a".repeat(64);
        let cases: &[(&str, &str, bool)] = &[
            ("plain", "ffmpeg", true),
            ("digits and separators", "tool-2_x", true),
            ("longest", &fits, true),
            ("empty", "", false),
            ("upper case", "Tool", false),
            ("space", "to ol", false),
            ("dot", "a.b", false),
            ("slash", "../x", false),
            ("too long", &long, false),
        ];
        for (name, text, ok) in cases {
            assert_eq!(PluginId::parse(text).is_ok(), *ok, "{name}");
        }
    }
}
