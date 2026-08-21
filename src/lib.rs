//! Datastar is a Rust implementation of the [Datastar](https://data-star.dev) SDK specification.

#![forbid(missing_docs)]
#![forbid(missing_debug_implementations)]

#[cfg(feature = "axum")]
pub mod axum;
#[cfg(feature = "rocket")]
pub mod rocket;
#[cfg(feature = "warp")]
pub mod warp;

pub mod execute_script;
pub mod patch_elements;
pub mod patch_signals;

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
#[expect(unused)]
struct ReadmeDoctests;

pub mod consts;

/// The prelude for the `datastar` crate
pub mod prelude {
    pub use crate::{
        DatastarEvent,
        consts::{ElementPatchMode, Namespace},
        execute_script::ExecuteScript,
        patch_elements::PatchElements,
        patch_signals::PatchSignals,
    };
}

use core::{
    fmt::{self, Display},
    time::Duration,
};

pub(crate) trait DatalineWriter {
    fn write_dataline(&mut self, args: fmt::Arguments<'_>) -> fmt::Result;
}

impl DatalineWriter for Vec<String> {
    fn write_dataline(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        self.push(args.to_string());
        Ok(())
    }
}

/// [`DatastarEvent`] is a struct that represents a generic Datastar event.
/// All Datastar events implement `Into<DatastarEvent>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DatastarEvent {
    /// `event` is the type of event.
    pub event: consts::EventType,
    /// `id` is can be used by the backend to replay events.
    /// This is part of the SSE spec and is used to tell the browser how to handle the event.
    /// For more details see <https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#id>
    pub id: Option<String>,
    /// `retry` is part of the SSE spec and is used to tell the browser how long to wait before reconnecting if the connection is lost.
    /// For more details see <https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#retry>
    pub retry: Duration,
    /// `data` is the data that is sent with the event.
    pub data: Vec<String>,
}

impl DatastarEvent {
    /// Creates a new Datastar event with the default SSE retry duration.
    pub fn new(
        event: consts::EventType,
        data: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            event,
            id: None,
            retry: Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION),
            data: data.into_iter().map(Into::into).collect(),
        }
    }

    /// Sets the event ID used by the browser for replay handling.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the reconnection delay for this event.
    pub fn retry(mut self, retry: Duration) -> Self {
        self.retry = retry;
        self
    }
}

impl Display for DatastarEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "event: {}", self.event.as_str())?;

        if let Some(id) = &self.id {
            write!(f, "\nid: {id}")?;
        }

        let millis = self.retry.as_millis();
        if millis != consts::DEFAULT_SSE_RETRY_DURATION as u128 {
            write!(f, "\nretry: {millis}")?;
        }

        for line in &self.data {
            write!(f, "\ndata: {line}")?;
        }

        write!(f, "\n\n")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::consts::EventType};

    #[test]
    fn formats_default_sse_metadata() {
        let event = DatastarEvent {
            event: EventType::PatchSignals,
            id: None,
            retry: Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION),
            data: vec!["signals {count: 1}".into()],
        };

        assert_eq!(
            event.to_string(),
            "event: datastar-patch-signals\ndata: signals {count: 1}\n\n"
        );
    }

    #[test]
    fn formats_custom_sse_metadata_and_multiline_data() {
        let event = DatastarEvent {
            event: EventType::PatchElements,
            id: Some("event-1".into()),
            retry: Duration::from_millis(2_500),
            data: vec![
                "elements <div>one</div>".into(),
                "elements <div>two</div>".into(),
            ],
        };

        assert_eq!(
            event.to_string(),
            concat!(
                "event: datastar-patch-elements\n",
                "id: event-1\n",
                "retry: 2500\n",
                "data: elements <div>one</div>\n",
                "data: elements <div>two</div>\n\n",
            )
        );
    }
}
