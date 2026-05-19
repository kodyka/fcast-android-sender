use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    #[default]
    Migration,
    GstPop,
}

impl BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Migration => "migration",
            Self::GstPop => "gst-pop",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "migration" => Some(Self::Migration),
            "gst-pop" | "gstpop" => Some(Self::GstPop),
            _ => None,
        }
    }
}
