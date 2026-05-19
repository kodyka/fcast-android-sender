use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

#[derive(Clone, Debug, Serialize)]
pub struct Request {
    pub id: String,
    pub method: String,
    pub params: Value,
}

impl Request {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response {
    #[serde(default)]
    pub id: Value,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

impl Response {
    pub fn id_as_str(&self) -> Option<String> {
        match &self.id {
            Value::String(value) => Some(value.clone()),
            Value::Null => None,
            other => Some(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gst-pop error ({}): {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum Event {
    StateChanged {
        pipeline_id: String,
        old_state: String,
        new_state: String,
    },
    Error {
        pipeline_id: String,
        message: String,
    },
    Unsupported {
        pipeline_id: String,
        message: String,
    },
    Eos {
        pipeline_id: String,
    },
    PipelineAdded {
        pipeline_id: String,
        description: String,
    },
    PipelineUpdated {
        pipeline_id: String,
        description: String,
    },
    PipelineRemoved {
        pipeline_id: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug)]
pub enum ClassifiedFrame {
    Response(Response),
    Event(Event),
    Garbage,
}

pub(crate) fn classify(text: &str) -> ClassifiedFrame {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(_) => return ClassifiedFrame::Garbage,
    };

    if value.get("event").is_some() {
        match serde_json::from_value::<Event>(value) {
            Ok(event) => ClassifiedFrame::Event(event),
            Err(_) => ClassifiedFrame::Event(Event::Other),
        }
    } else if value.get("id").is_some() {
        match serde_json::from_value::<Response>(value) {
            Ok(response) => ClassifiedFrame::Response(response),
            Err(_) => ClassifiedFrame::Garbage,
        }
    } else {
        ClassifiedFrame::Garbage
    }
}
