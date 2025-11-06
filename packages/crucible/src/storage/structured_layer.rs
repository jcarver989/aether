use std::{
    fs::File,
    io::{BufWriter, Write},
    sync::{Arc, Mutex},
};

use serde_json::json;
use tracing::{
    Event, Subscriber,
    span::{Attributes, Id},
};
use tracing_serde_structured::AsSerde;
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

/// A tracing layer that writes structured events to a file using tracing-serde-structured
pub struct StructuredLayer {
    writer: Arc<Mutex<BufWriter<File>>>,
}

impl StructuredLayer {
    pub fn new(file: File) -> Self {
        Self {
            writer: Arc::new(Mutex::new(BufWriter::new(file))),
        }
    }

    fn write_json(&self, value: serde_json::Value) {
        if let Ok(mut writer) = self.writer.lock() {
            if let Ok(json_str) = serde_json::to_string(&value) {
                let _ = writeln!(writer, "{}", json_str);
                let _ = writer.flush();
            }
        }
    }
}

impl<S> Layer<S> for StructuredLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, _ctx: Context<'_, S>) {
        let json = json!({
            "new_span": {
                "attributes": attrs.as_serde(),
                "id": id.as_serde(),
            }
        });
        self.write_json(json);
    }

    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let json = json!({
            "event": event.as_serde(),
        });
        self.write_json(json);
    }

    fn on_enter(&self, id: &Id, _ctx: Context<'_, S>) {
        let json = json!({
            "enter": {
                "id": id.as_serde(),
            }
        });
        self.write_json(json);
    }

    fn on_exit(&self, id: &Id, _ctx: Context<'_, S>) {
        let json = json!({
            "exit": {
                "id": id.as_serde(),
            }
        });
        self.write_json(json);
    }

    fn on_close(&self, id: Id, _ctx: Context<'_, S>) {
        let json = json!({
            "close": {
                "id": id.as_serde(),
            }
        });
        self.write_json(json);
    }
}
