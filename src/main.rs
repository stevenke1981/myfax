//! MyFax desktop entry point: AcmeUI-Native `Application` glue that connects
//! platform events → [`UiRuntime`] → [`Controller`] and renders `views::root`.
#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use acme_core::Scene;
use acme_platform::{
    Application, FrameContext, PlatformEvent, PlatformKey, Waker, WindowConfig, WindowId,
    WindowRequest, run,
};
use myfax::app::controller::WakeFn;
use myfax::app::platform::SystemPlatform;
use myfax::app::{Controller, Msg};
use myfax::config;
use myfax::i18n::{t, tf};
use myfax::ui::runtime::{FrameInput, Modifiers, UiEvent, UiRuntime};
use myfax::ui::theme::app_theme;
use myfax::ui::views;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DEFAULT_WIDTH: f64 = 1280.0;
const DEFAULT_HEIGHT: f64 = 800.0;
const MIN_WIDTH: f64 = 1024.0;
const MIN_HEIGHT: f64 = 680.0;
const BACKGROUND_POLL: Duration = Duration::from_millis(750);
const LEFT_BUTTON: u16 = 0;

struct MyFaxApp {
    controller: Controller,
    runtime: UiRuntime<Msg>,
    modifiers: Modifiers,
    title: String,
    pending_requests: Vec<WindowRequest>,
}

impl MyFaxApp {
    fn new(controller: Controller) -> Self {
        Self {
            controller,
            runtime: UiRuntime::new(),
            modifiers: Modifiers::default(),
            title: String::new(),
            pending_requests: Vec::new(),
        }
    }

    fn dispatch(&mut self, events: Vec<UiEvent<Msg>>) -> bool {
        let changed = !events.is_empty();
        for event in events {
            self.controller.on_ui_event(event);
        }
        changed
    }

    /// Window title shows the unread count, e.g. "MyFax (3)".
    fn refresh_title(&mut self) {
        let state = &self.controller.state;
        let unread = state.counts.unread;
        let title = if unread > 0 {
            tf(
                state.lang,
                "shell.window_title_unread",
                &[("count", &unread.to_string())],
            )
        } else {
            t(state.lang, "shell.window_title").to_string()
        };
        if title != self.title {
            self.title = title.clone();
            self.pending_requests.push(WindowRequest::SetTitle(title));
        }
    }
}

impl Application for MyFaxApp {
    fn window_config(&self) -> WindowConfig {
        WindowConfig {
            title: t(self.controller.state.lang, "shell.window_title").to_string(),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            min_width: Some(MIN_WIDTH),
            min_height: Some(MIN_HEIGHT),
            ..WindowConfig::default()
        }
    }

    fn minimum_window_size(&self) -> Option<(f64, f64)> {
        Some((MIN_WIDTH, MIN_HEIGHT))
    }

    fn on_waker(&mut self, waker: Waker) {
        let shared = Arc::new(Mutex::new(waker));
        let wake: WakeFn = Arc::new(move || {
            if let Ok(waker) = shared.lock() {
                waker.wake();
            }
        });
        self.controller.start_watcher(wake);
    }

    fn wake(&mut self) -> bool {
        self.controller.poll_background()
    }

    fn background_poll_interval(&self) -> Option<Duration> {
        Some(BACKGROUND_POLL)
    }

    fn background_tick(&mut self) -> bool {
        let polled = self.controller.poll_background();
        let ticked = self.controller.tick();
        polled || ticked
    }

    fn take_window_requests(&mut self, _window: WindowId) -> Vec<WindowRequest> {
        std::mem::take(&mut self.pending_requests)
    }

    fn ime_cursor_area(&self, _window: WindowId) -> Option<[f32; 4]> {
        self.runtime.ime_cursor_area()
    }

    fn on_gpu_recovered(&mut self, _window: WindowId) {
        self.runtime.clear_gpu_caches();
    }

    fn event(&mut self, event: PlatformEvent) -> bool {
        match event {
            PlatformEvent::Resized { .. } | PlatformEvent::MaximizedChanged { .. } => true,
            PlatformEvent::PointerMoved { x, y, .. } => self.runtime.pointer_moved(x, y),
            PlatformEvent::PointerLeft { .. } | PlatformEvent::CursorLeft { .. } => {
                self.runtime.pointer_left()
            }
            PlatformEvent::PointerButton {
                pressed,
                x,
                y,
                button,
                shift,
                ctrl,
                alt,
                ..
            } => {
                self.modifiers = Modifiers { shift, ctrl, alt };
                if button != LEFT_BUTTON {
                    return false;
                }
                let events = self.runtime.pointer_button(pressed, x, y);
                self.dispatch(events);
                true
            }
            PlatformEvent::Scroll {
                delta_x,
                delta_y,
                lines,
                x,
                y,
                ..
            } => {
                // Shift + vertical wheel pans horizontally (zoomed preview).
                let (dx, dy) = if self.modifiers.shift && delta_x == 0.0 {
                    (delta_y, 0.0)
                } else {
                    (delta_x, delta_y)
                };
                self.runtime.scroll(dx, dy, lines, x, y)
            }
            PlatformEvent::Key {
                key,
                pressed,
                shift,
                ctrl,
                alt,
                text,
                ..
            } => {
                self.modifiers = Modifiers { shift, ctrl, alt };
                let events = self
                    .runtime
                    .key(key, pressed, self.modifiers, text.as_deref());
                let had_events = self.dispatch(events);
                had_events || (pressed && key != PlatformKey::Other) || text.is_some()
            }
            PlatformEvent::ImePreedit { text, .. } => self.runtime.ime_preedit(&text),
            PlatformEvent::ImeCommit { text, .. } => {
                let events = self.runtime.ime_commit(&text);
                self.dispatch(events);
                true
            }
            PlatformEvent::FileDropped { paths, .. } => {
                let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
                self.controller.files_dropped(paths);
                true
            }
            PlatformEvent::WindowCloseRequested(_) => {
                self.controller.shutdown();
                false
            }
            _ => false,
        }
    }

    fn frame(&mut self, context: FrameContext) -> Scene {
        self.controller.tick();
        self.refresh_title();
        let theme = app_theme(self.controller.state.theme);
        let root = views::root(&self.controller.state);
        let input = FrameInput {
            logical_width: context.logical_width,
            logical_height: context.logical_height,
            scale_factor: context.scale_factor,
            theme: &theme,
            images: self.controller.images(),
        };
        self.runtime.render(&root, input)
    }
}

fn init_logging() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,myfax=info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn main() {
    init_logging();
    let controller = match Controller::new(
        config::settings_path(),
        config::database_path(),
        Box::new(SystemPlatform),
    ) {
        Ok(controller) => controller,
        Err(error) => {
            eprintln!("MyFax: start-up failed: {error:#}");
            std::process::exit(1);
        }
    };
    if let Err(error) = run(MyFaxApp::new(controller)) {
        eprintln!("MyFax: {error}");
        std::process::exit(1);
    }
}
