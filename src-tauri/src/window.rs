//! Executes the actions of `mango_app::WindowPolicy`. All decisions live in
//! the policy; this only feeds it events and touches windows.
//!
//! The launcher window is destroyed (not hidden) while a game runs with
//! `hideOnLaunch` on, so the WebKit processes go away; it is created again
//! from the `tauri.conf.json` window entry, so label, size and title match.

use std::sync::Mutex;
use std::time::Duration;

use mango_app::{
    Activity, AppService, Event, PolicyInput, WindowAction, WindowPolicy, WindowStateEvent,
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

pub const MAIN_LABEL: &str = "main";

/// Window commands are for the launcher's own webview only.
pub fn main_only(window: &tauri::WebviewWindow) -> Result<(), mango_app::AppError> {
    if window.label() == MAIN_LABEL {
        Ok(())
    } else {
        Err(mango_app::AppError::new(
            "window.forbidden",
            "Only the main window may control itself",
        ))
    }
}

pub fn resize_direction(d: mango_app::ResizeDirection) -> tauri_runtime::ResizeDirection {
    use mango_app::ResizeDirection as R;
    use tauri_runtime::ResizeDirection as T;
    match d {
        R::North => T::North,
        R::South => T::South,
        R::East => T::East,
        R::West => T::West,
        R::NorthEast => T::NorthEast,
        R::NorthWest => T::NorthWest,
        R::SouthEast => T::SouthEast,
        R::SouthWest => T::SouthWest,
    }
}

/// The last `window:state` that went out (only changes are emitted).
#[derive(Default)]
pub struct LastState(Mutex<Option<WindowStateEvent>>);

/// Emit `window:state` when maximized / fullscreen / focused changed.
pub fn emit_state_if_changed(app: &AppHandle, window: &tauri::Window) {
    let state = WindowStateEvent {
        maximized: window.is_maximized().unwrap_or(false),
        fullscreen: window.is_fullscreen().unwrap_or(false),
        focused: window.is_focused().unwrap_or(true),
    };
    let Some(last) = app.try_state::<LastState>() else {
        return;
    };
    {
        let mut slot = last.0.lock().unwrap_or_else(|e| e.into_inner());
        if slot.as_ref() == Some(&state) {
            return;
        }
        *slot = Some(state.clone());
    }
    let event = Event::WindowState(state);
    let _ = app.emit(event.name(), event.payload());
}

/// Managed state: the policy and the queue that feeds it from other threads.
pub struct Lifecycle {
    policy: Mutex<WindowPolicy>,
    tx: UnboundedSender<PolicyInput>,
}

impl Lifecycle {
    /// Queue an input; the pump task evaluates it in order.
    pub fn send(&self, input: PolicyInput) {
        let _ = self.tx.send(input);
    }

    fn exit_pending(&self) -> bool {
        self.policy
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .exit_pending()
    }
}

/// Registers the managed state and starts the pump. Call from `setup`
/// (the async runtime is up by then).
pub fn install(app: &AppHandle) {
    let (tx, rx) = unbounded_channel();
    app.manage(LastState::default());
    app.manage(Lifecycle {
        policy: Mutex::new(WindowPolicy::new()),
        tx,
    });
    tauri::async_runtime::spawn(pump(app.clone(), rx));
}

/// The queue in order; while a quit is held back it also re-checks every
/// second (an install can end without any event).
async fn pump(app: AppHandle, mut rx: UnboundedReceiver<PolicyInput>) {
    loop {
        let pending = app
            .try_state::<Lifecycle>()
            .is_some_and(|l| l.exit_pending());
        let input = if pending {
            match tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
                Ok(Some(i)) => i,
                Ok(None) => return,
                Err(_) => PolicyInput::Tick,
            }
        } else {
            match rx.recv().await {
                Some(i) => i,
                None => return,
            }
        };
        dispatch(&app, input, false);
    }
}

/// `RunEvent::ExitRequested`. `true` = call `api.prevent_exit()`.
pub fn on_exit_requested(app: &AppHandle, programmatic: bool) -> bool {
    dispatch(app, PolicyInput::ExitRequested { programmatic }, true)
}

/// Evaluate `input` and run the actions. Returns whether the exit must be
/// prevented (only meaningful for `ExitRequested`). `in_exit_event`: the
/// quit is already under way, so `Exit` just lets it proceed.
fn dispatch(app: &AppHandle, input: PolicyInput, in_exit_event: bool) -> bool {
    let (Some(svc), Some(life)) = (app.try_state::<AppService>(), app.try_state::<Lifecycle>())
    else {
        return false;
    };
    let activity: Activity = svc.activity();
    let actions = life
        .policy
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .handle(input, &activity);
    let mut prevent = false;
    for action in actions {
        match action {
            WindowAction::PreventExit => prevent = true,
            WindowAction::Exit { apply_update } => {
                if apply_update {
                    if let Err(e) = svc.update_install_on_exit() {
                        eprintln!("staged update not installed: {e}");
                    }
                }
                if !in_exit_event {
                    app.exit(0);
                }
            }
            window_action => run_on_main(app, window_action),
        }
    }
    prevent
}

fn run_on_main(app: &AppHandle, action: WindowAction) {
    let handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || match action {
        WindowAction::Show => show_main(&handle),
        WindowAction::Destroy => {
            if let Some(w) = handle.get_webview_window(MAIN_LABEL) {
                let _ = w.destroy();
            }
        }
        WindowAction::Minimize => {
            if let Some(w) = handle.get_webview_window(MAIN_LABEL) {
                let _ = w.minimize();
            }
        }
        WindowAction::PreventExit | WindowAction::Exit { .. } => {}
    }) {
        eprintln!("window action not run: {e}");
    }
}

/// Raise the window, or build it again from the config entry.
fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let Some(cfg) = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN_LABEL)
        .cloned()
    else {
        return;
    };
    let built = WebviewWindowBuilder::from_config(app, &cfg).and_then(|b| b.build());
    if let Err(e) = built {
        eprintln!("could not re-create the main window: {e}");
    }
}
