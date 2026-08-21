use {
    core::time::Duration,
    datastar::{
        consts::{ElementPatchMode, Namespace},
        prelude::{DatastarEvent, ExecuteScript, PatchElements, PatchSignals},
    },
    indexmap::IndexMap,
    serde::Deserialize,
    serde_json::Value,
};

#[derive(Deserialize)]
pub(crate) struct TestCase {
    pub events: Vec<TestCaseEvent>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub(crate) enum TestCaseEvent {
    #[serde(alias = "executeScript")]
    ExecuteScript {
        script: String,
        #[serde(alias = "eventId")]
        event_id: Option<String>,
        #[serde(alias = "retryDuration")]
        retry_duration: Option<u64>,
        attributes: Option<IndexMap<String, Value>>,
        #[serde(alias = "autoRemove")]
        auto_remove: Option<bool>,
    },
    #[serde(rename = "patchElements")]
    PatchElements {
        elements: Option<String>,
        #[serde(alias = "eventId")]
        event_id: Option<String>,
        #[serde(alias = "retryDuration")]
        retry_duration: Option<u64>,
        selector: Option<String>,
        mode: Option<String>,
        #[serde(alias = "useViewTransition")]
        use_view_transition: Option<bool>,
        #[serde(alias = "viewTransitionSelector")]
        view_transition_selector: Option<String>,
        namespace: Option<String>,
    },
    #[serde(rename = "patchSignals")]
    PatchSignals {
        signals: Option<IndexMap<String, Value>>,
        #[serde(alias = "signals-raw")]
        signals_raw: Option<String>,
        #[serde(alias = "eventId")]
        event_id: Option<String>,
        #[serde(alias = "retryDuration")]
        retry_duration: Option<u64>,
        #[serde(alias = "onlyIfMissing")]
        only_if_missing: Option<bool>,
    },
}

impl TestCaseEvent {
    pub(crate) fn into_datastar_event(self) -> DatastarEvent {
        match self {
            Self::ExecuteScript {
                script,
                event_id,
                retry_duration,
                attributes,
                auto_remove,
            } => {
                let mut event = ExecuteScript::new(script);
                if let Some(event_id) = event_id {
                    event = event.id(event_id);
                }
                if let Some(retry_duration) = retry_duration {
                    event = event.retry(Duration::from_millis(retry_duration));
                }
                if let Some(auto_remove) = auto_remove {
                    event = event.auto_remove(auto_remove);
                }
                if let Some(attributes) = attributes {
                    event = event.attributes(attributes.into_iter().map(|(key, value)| {
                        format!("{key}=\"{}\"", value.to_string().trim_matches('"'))
                    }));
                }
                event.into_datastar_event()
            }
            Self::PatchElements {
                elements,
                event_id,
                retry_duration,
                selector,
                mode,
                use_view_transition,
                view_transition_selector,
                namespace,
            } => {
                let mode = element_patch_mode(mode.as_deref());
                let mut event = if mode == ElementPatchMode::Remove {
                    PatchElements::new_remove(selector.clone().unwrap_or_default())
                } else {
                    PatchElements::new(elements.unwrap_or_default())
                };
                if let Some(event_id) = event_id {
                    event = event.id(event_id);
                }
                if let Some(retry_duration) = retry_duration {
                    event = event.retry(Duration::from_millis(retry_duration));
                }
                if let Some(selector) = selector {
                    event = event.selector(selector);
                }
                event = event.mode(mode);
                if let Some(use_view_transition) = use_view_transition {
                    event = event.use_view_transition(use_view_transition);
                }
                if let Some(view_transition_selector) = view_transition_selector {
                    event = event.view_transition_selector(view_transition_selector);
                }
                event = event.namespace(element_namespace(namespace.as_deref()));
                event.into_datastar_event()
            }
            Self::PatchSignals {
                signals,
                signals_raw,
                event_id,
                retry_duration,
                only_if_missing,
            } => {
                let signals = signals_raw.unwrap_or_else(|| {
                    signals
                        .map(|signals| serde_json::to_string(&signals).unwrap_or_default())
                        .unwrap_or_default()
                });
                let mut event = PatchSignals::new(signals);
                if let Some(event_id) = event_id {
                    event = event.id(event_id);
                }
                if let Some(retry_duration) = retry_duration {
                    event = event.retry(Duration::from_millis(retry_duration));
                }
                if let Some(only_if_missing) = only_if_missing {
                    event = event.only_if_missing(only_if_missing);
                }
                event.into_datastar_event()
            }
        }
    }
}

fn element_patch_mode(mode: Option<&str>) -> ElementPatchMode {
    match mode {
        Some("inner") => ElementPatchMode::Inner,
        Some("remove") => ElementPatchMode::Remove,
        Some("replace") => ElementPatchMode::Replace,
        Some("prepend") => ElementPatchMode::Prepend,
        Some("append") => ElementPatchMode::Append,
        Some("before") => ElementPatchMode::Before,
        Some("after") => ElementPatchMode::After,
        _ => ElementPatchMode::Outer,
    }
}

fn element_namespace(namespace: Option<&str>) -> Namespace {
    match namespace {
        Some("svg") => Namespace::Svg,
        Some("mathml") => Namespace::MathMl,
        _ => Namespace::Html,
    }
}
