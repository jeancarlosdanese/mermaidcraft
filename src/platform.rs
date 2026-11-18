//! Operating system events: documents handed over by the Finder (macOS Apple events).

use std::path::PathBuf;

/// Documents the system asks the running app to open.
pub struct OpenEvents {
    #[cfg(target_os = "macos")]
    inbox: Option<fmv_macos_events::Inbox>,
}

/// Keeps the system registration alive until the event loop ends.
pub struct Guard {
    #[cfg(target_os = "macos")]
    _registration: Option<fmv_macos_events::Registration>,
}

impl OpenEvents {
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub fn none() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            inbox: None,
        }
    }

    /// Files to open since the last call, and whether a quit was requested.
    pub fn drain(&self) -> (Vec<PathBuf>, bool) {
        #[allow(unused_mut)]
        let mut files = Vec::new();
        #[allow(unused_mut)]
        let mut quit = false;
        #[cfg(target_os = "macos")]
        if let Some(inbox) = &self.inbox {
            for e in inbox.drain() {
                match e {
                    fmv_macos_events::Event::Open(p) => files.extend(p),
                    fmv_macos_events::Event::Quit => quit = true,
                }
            }
        }
        (files, quit)
    }

    /// Wakes the app when an event arrives while it is idle.
    #[allow(unused_variables)]
    pub fn set_wake(&self, ctx: &egui::Context) {
        #[cfg(target_os = "macos")]
        if let Some(inbox) = &self.inbox {
            let ctx = ctx.clone();
            inbox.set_wake(move || ctx.request_repaint());
        }
    }
}

/// Starts listening for documents. On macOS this must happen before the event loop starts, so
/// the document that launched the app is not missed.
pub fn listen() -> (Guard, OpenEvents) {
    #[cfg(target_os = "macos")]
    {
        let (reg, inbox) = fmv_macos_events::Registration::install();
        (Guard { _registration: Some(reg) }, OpenEvents { inbox: Some(inbox) })
    }
    #[cfg(not(target_os = "macos"))]
    {
        (Guard {}, OpenEvents::none())
    }
}
