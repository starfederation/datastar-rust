//! [`ExecuteScript`] executes JavaScript in the browser.
//!
//! This is sugar for `PatchElements` specifically for executing scripts.

use {
    crate::{
        DatalineWriter, DatastarEvent,
        consts::{self, ElementPatchMode},
    },
    core::{fmt, time::Duration},
};

/// [`ExecuteScript`] executes JavaScript in the browser
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecuteScript {
    /// `id` can be used by the backend to replay events.
    /// This is part of the SSE spec and is used to tell the browser how to handle the event.
    /// For more details see <https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#id>
    pub id: Option<String>,
    /// `retry` is part of the SSE spec and is used to tell the browser how long to wait before reconnecting if the connection is lost.
    /// For more details see <https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events#retry>
    pub retry: Duration,
    /// `script` is a string that represents the JavaScript to be executed by the browser.
    pub script: String,
    /// Whether to remove the script after execution, if not provided the Datastar client side will default to `true`.
    pub auto_remove: Option<bool>,
    /// A list of attributes to add to the script element, if not provided the Datastar client side will default to `type="module"`.
    /// Each item in the array ***must*** be properly formatted.
    pub attributes: Vec<String>,
}

impl ExecuteScript {
    /// Creates a new [`ExecuteScript`] event with the given script.
    pub fn new(script: impl Into<String>) -> Self {
        Self {
            id: None,
            retry: Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION),
            script: script.into(),
            auto_remove: Default::default(),
            attributes: Default::default(),
        }
    }

    /// Sets the `id` of the [`ExecuteScript`] event.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the `retry` of the [`ExecuteScript`] event.
    pub fn retry(mut self, retry: Duration) -> Self {
        self.retry = retry;
        self
    }

    /// Sets the `script` of the [`ExecuteScript`] event.
    pub fn auto_remove(mut self, auto_remove: bool) -> Self {
        self.auto_remove = Some(auto_remove);
        self
    }

    /// Sets the `attribute` of the [`ExecuteScript`] event.
    pub fn attributes(mut self, attributes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.attributes = attributes.into_iter().map(Into::into).collect();
        self
    }

    /// Converts this [`ExecuteScript`] into a [`DatastarEvent`].
    #[inline]
    pub fn into_datastar_event(mut self) -> DatastarEvent {
        let id = self.id.take();
        self.convert_to_datastar_event_inner(id)
    }

    /// Copy this [`ExecuteScript`] as a [`DatastarEvent`].
    #[inline]
    pub fn as_datastar_event(&self) -> DatastarEvent {
        self.convert_to_datastar_event_inner(self.id.clone())
    }

    fn convert_to_datastar_event_inner(&self, id: Option<String>) -> DatastarEvent {
        let mut data: Vec<String> = Vec::new();
        self.write_datalines(&mut data)
            .expect("writing datalines to a Vec cannot fail");

        DatastarEvent {
            event: consts::EventType::PatchElements,
            id,
            retry: self.retry,
            data,
        }
    }

    pub(crate) fn write_datalines(&self, writer: &mut impl DatalineWriter) -> fmt::Result {
        writer.write_dataline(format_args!("{} body", consts::SELECTOR_DATALINE_LITERAL))?;

        writer.write_dataline(format_args!(
            "{} {}",
            consts::MODE_DATALINE_LITERAL,
            ElementPatchMode::Append.as_str(),
        ))?;

        let mut script_lines = self.script.lines().peekable();
        let first_line = script_lines.next().unwrap_or_default();
        let close = script_lines.peek().is_none();

        writer.write_dataline(format_args!(
            "{} {}",
            consts::ELEMENTS_DATALINE_LITERAL,
            ScriptOpeningLine {
                script: self,
                first_line,
                close,
            }
        ))?;

        while let Some(line) = script_lines.next() {
            let closing_tag = if script_lines.peek().is_none() {
                "</script>"
            } else {
                ""
            };
            writer.write_dataline(format_args!(
                "{} {}{}",
                consts::ELEMENTS_DATALINE_LITERAL,
                line,
                closing_tag
            ))?;
        }

        Ok(())
    }
}

struct ScriptOpeningLine<'a> {
    script: &'a ExecuteScript,
    first_line: &'a str,
    close: bool,
}

impl fmt::Display for ScriptOpeningLine<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<script")?;

        if self.script.auto_remove.unwrap_or(true) {
            f.write_str(r##" data-effect="el.remove()""##)?;
        }

        for attribute in &self.script.attributes {
            f.write_str(" ")?;
            f.write_str(attribute)?;
        }

        f.write_str(">")?;
        f.write_str(self.first_line)?;

        if self.close {
            f.write_str("</script>")?;
        }

        Ok(())
    }
}

impl From<&ExecuteScript> for DatastarEvent {
    #[inline]
    fn from(val: &ExecuteScript) -> Self {
        val.as_datastar_event()
    }
}

impl From<ExecuteScript> for DatastarEvent {
    #[inline]
    fn from(val: ExecuteScript) -> Self {
        val.into_datastar_event()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_data() -> Vec<String> {
        vec![
            "selector body".into(),
            "mode append".into(),
            "elements <script type=\"module\">first".into(),
            "elements second</script>".into(),
        ]
    }

    #[test]
    fn serializes_script_options_and_conversions() {
        let script = ExecuteScript::new("first\nsecond")
            .id("script-1")
            .retry(Duration::from_millis(2_000))
            .auto_remove(false)
            .attributes([r#"type="module""#]);

        let borrowed = script.as_datastar_event();
        assert_eq!(borrowed.id.as_deref(), Some("script-1"));
        assert_eq!(borrowed.retry, Duration::from_millis(2_000));
        assert_eq!(borrowed.data, expected_data());
        assert_eq!(DatastarEvent::from(&script).data, expected_data());
        assert_eq!(DatastarEvent::from(script).data, expected_data());
    }

    #[test]
    fn serializes_empty_script_with_defaults() {
        let event = ExecuteScript::new("").into_datastar_event();

        assert_eq!(
            event.data,
            [
                "selector body",
                "mode append",
                r#"elements <script data-effect="el.remove()"></script>"#,
            ]
        );
    }
}
