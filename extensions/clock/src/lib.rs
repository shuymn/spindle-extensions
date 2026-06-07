//! Clock label projection logic.
//!
//! This extension mirrors the former `SketchyBar` clock plugin by producing a
//! generic `SketchyBar` message request for the current local time.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

use chrono::{DateTime, Local};
use serde_json::json;
use spindle_extension_sdk::ActionOutputEvent;

/// Event emitted when a generic `SketchyBar` clock message should be sent.
pub const OUTPUT_EVENT: &str = "clock.sketchybar.message.requested";

const CLOCK_LABEL_FORMAT: &str = "%m/%d %a %H:%M:%S";

/// Generic `SketchyBar` message request produced by this workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SketchybarMessageRequest {
    args: Vec<String>,
}

impl SketchybarMessageRequest {
    /// Return command-line style `SketchyBar` arguments.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Convert this message request into a spindle event.
    #[must_use]
    pub fn into_event(self) -> ActionOutputEvent {
        ActionOutputEvent::new(OUTPUT_EVENT).with_data(json!({
            "args": self.args,
        }))
    }
}

/// Build a `SketchyBar` message request for a clock item.
#[must_use]
pub fn build_clock_message(item: &str, now: DateTime<Local>) -> SketchybarMessageRequest {
    SketchybarMessageRequest {
        args: vec![
            String::from("--set"),
            String::from(item),
            format!("label={}", now.format(CLOCK_LABEL_FORMAT)),
        ],
    }
}

/// Format a timestamp like `LC_TIME=C date '+%m/%d %a %H:%M:%S'`.
#[must_use]
pub fn format_clock_label(now: DateTime<Local>) -> String {
    now.format(CLOCK_LABEL_FORMAT).to_string()
}
