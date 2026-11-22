use serde_json::Value;

/// A library-emitted event, decoupled from any UI framework.
///
/// `name` is the event name the desktop observer matches on (mirroring the
/// tauri_specta serde tag), and `payload` is the serialized camelCase payload.
pub struct LibraryEvent {
    pub name: &'static str,
    pub payload: Value,
}

/// Receives [`LibraryEvent`]s produced by core logic (e.g. the downloader).
///
/// Implementors (such as the desktop shell) translate these into their own
/// event system (e.g. tauri_specta events).
pub trait EventObserver: Send + Sync {
    fn on_event(&self, event: &LibraryEvent);
}
