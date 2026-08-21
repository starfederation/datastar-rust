//! Rocket integration for Datastar.

use {
    crate::{
        DatastarEvent,
        prelude::{ExecuteScript, PatchElements, PatchSignals},
    },
    rocket::response::stream::Event,
    std::fmt::Write,
};

impl PatchElements {
    /// Write this [`PatchElements`] into a Rocket SSE [`Event`].
    pub fn write_as_rocket_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_rocket_sse_event()
    }
}

impl From<PatchElements> for Event {
    fn from(value: PatchElements) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl From<&PatchElements> for Event {
    fn from(value: &PatchElements) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl PatchSignals {
    /// Write this [`PatchSignals`] into a Rocket SSE [`Event`].
    pub fn write_as_rocket_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_rocket_sse_event()
    }
}

impl From<PatchSignals> for Event {
    fn from(value: PatchSignals) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl From<&PatchSignals> for Event {
    fn from(value: &PatchSignals) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl ExecuteScript {
    /// Write this [`ExecuteScript`] into a Rocket SSE [`Event`].
    pub fn write_as_rocket_sse_event(&self) -> Event {
        self.as_datastar_event().write_as_rocket_sse_event()
    }
}

impl From<ExecuteScript> for Event {
    fn from(value: ExecuteScript) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl From<&ExecuteScript> for Event {
    fn from(value: &ExecuteScript) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl DatastarEvent {
    /// Turn this [`DatastarEvent`] into a Rocket SSE [`Event`].
    pub fn write_as_rocket_sse_event(&self) -> Event {
        let mut data = String::with_capacity(
            (self.data.iter().map(|s| s.len()).sum::<usize>() + self.data.len()).saturating_sub(1),
        );

        let mut sep = "";
        for line in self.data.iter() {
            // Assumption: std::fmt::write does not fail ever for [`String`].
            let _ = write!(&mut data, "{sep}{line}");
            sep = "\n";
        }

        let event = Event::data(data)
            .event(self.event.as_str().to_owned())
            .with_retry(self.retry);

        match self.id.as_deref() {
            Some(id) => event.id(id.to_owned()),
            None => event,
        }
    }
}

impl From<DatastarEvent> for Event {
    fn from(value: DatastarEvent) -> Self {
        value.write_as_rocket_sse_event()
    }
}

impl From<&DatastarEvent> for Event {
    fn from(value: &DatastarEvent) -> Self {
        value.write_as_rocket_sse_event()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::consts::{self, EventType},
        core::time::Duration,
    };

    fn assert_event(event: Event, expected: &Event) {
        assert_eq!(&event, expected);
    }

    #[test]
    fn writes_patch_elements_and_conversions() {
        let patch = PatchElements::new("<div>hello</div>")
            .id("elements-1")
            .retry(Duration::from_millis(2_500));
        let expected = Event::data("elements <div>hello</div>")
            .event("datastar-patch-elements")
            .with_retry(Duration::from_millis(2_500))
            .id("elements-1");

        assert_event(patch.write_as_rocket_sse_event(), &expected);
        assert_event(Event::from(&patch), &expected);
        assert_event(Event::from(patch), &expected);
    }

    #[test]
    fn writes_patch_signals_and_conversions() {
        let patch = PatchSignals::new("{count: 1}").only_if_missing(true);
        let expected = Event::data("onlyIfMissing true\nsignals {count: 1}")
            .event("datastar-patch-signals")
            .with_retry(Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION));

        assert_event(patch.write_as_rocket_sse_event(), &expected);
        assert_event(Event::from(&patch), &expected);
        assert_event(Event::from(patch), &expected);
    }

    #[test]
    fn writes_execute_script_and_conversions() {
        let script = ExecuteScript::new("console.log('hello')");
        let expected = Event::data(concat!(
            "selector body\n",
            "mode append\n",
            "elements <script data-effect=\"el.remove()\">",
            "console.log('hello')</script>",
        ))
        .event("datastar-patch-elements")
        .with_retry(Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION));

        assert_event(script.write_as_rocket_sse_event(), &expected);
        assert_event(Event::from(&script), &expected);
        assert_event(Event::from(script), &expected);
    }

    #[test]
    fn writes_generic_events_and_conversions() {
        let event = DatastarEvent {
            event: EventType::PatchSignals,
            id: Some("signals-1".into()),
            retry: Duration::from_millis(2_500),
            data: vec!["signals {count: 1}".into()],
        };
        let expected = Event::data("signals {count: 1}")
            .event("datastar-patch-signals")
            .with_retry(Duration::from_millis(2_500))
            .id("signals-1");

        assert_event(event.write_as_rocket_sse_event(), &expected);
        assert_event(Event::from(&event), &expected);
        assert_event(Event::from(event), &expected);
    }

    #[test]
    fn writes_empty_generic_data() {
        let event = DatastarEvent {
            event: EventType::PatchElements,
            id: None,
            retry: Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION),
            data: Vec::new(),
        };
        let expected = Event::data("")
            .event("datastar-patch-elements")
            .with_retry(Duration::from_millis(consts::DEFAULT_SSE_RETRY_DURATION));

        assert_event(event.write_as_rocket_sse_event(), &expected);
    }
}
