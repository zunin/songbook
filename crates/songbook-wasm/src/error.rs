use std::fmt;

/// Errors returned by the browser session API. Messages are plain strings so
/// they can cross the wasm-bindgen boundary without extra serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    EmptyInput,
    NoSections,
    UnknownAction(String),
    UnknownKey(String),
    NotEditing,
    InvalidChar(String),
    NoTabSplitPlan,
    TabSplit(String),
    Render(String),
    Compose(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(formatter, "input text is empty"),
            Self::NoSections => write!(
                formatter,
                "no song sections found; the input may be only noise or metadata"
            ),
            Self::UnknownAction(action) => write!(formatter, "unknown action: {action}"),
            Self::UnknownKey(key) => write!(formatter, "unknown key: {key}"),
            Self::NotEditing => write!(
                formatter,
                "text can only be inserted while editing a line or metadata field"
            ),
            Self::InvalidChar(value) => {
                write!(formatter, "expected exactly one character, got: {value:?}")
            }
            Self::NoTabSplitPlan => write!(
                formatter,
                "no tab split plan; preview the current block before applying"
            ),
            Self::TabSplit(message) => write!(formatter, "tab splitting failed: {message}"),
            Self::Render(message) => write!(formatter, "rendering failed: {message}"),
            Self::Compose(message) => write!(formatter, "book composition failed: {message}"),
        }
    }
}

impl std::error::Error for SessionError {}
