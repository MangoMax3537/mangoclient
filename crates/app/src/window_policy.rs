//! Window lifecycle as a pure state machine. The Tauri shell feeds it events
//! plus an [`Activity`] snapshot and executes the returned actions; nothing
//! here touches a window.
//!
//! - `hideOnLaunch` on: when a game reports `running` the main window is
//!   DESTROYED (the process, the service and the game keep running) and
//!   created again when that game exits or a second start asks for it.
//! - `hideOnLaunch` off, `keepLauncherOpen` off: the window is minimized.
//! - Closing the last window while a game runs or something is downloading or
//!   installing does not quit; the quit is held back and happens when the
//!   work is done. A normal quit applies a staged update.

use crate::shell::Activity;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyInput {
    /// A game reported `running`.
    GameRunning { profile_id: String },
    /// A game stopped or crashed.
    GameEnded { profile_id: String },
    /// The user started the executable again.
    SecondInstance,
    /// `RunEvent::ExitRequested`; `programmatic` = it came from `exit()` (the
    /// updater's restart, ...), not from the last window closing.
    ExitRequested { programmatic: bool },
    /// Periodic re-check while a quit is being held back.
    Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    /// Show and focus the window; create it first when it was destroyed.
    Show,
    /// Destroy the main window (frees the WebKit processes).
    Destroy,
    Minimize,
    /// Keep the process alive (`api.prevent_exit()`).
    PreventExit,
    /// Quit now. `apply_update`: hand a staged update to the installer first.
    Exit {
        apply_update: bool,
    },
}

#[derive(Debug, Default)]
pub struct WindowPolicy {
    /// The window was destroyed by this policy and is not back yet.
    destroyed: bool,
    /// Games whose `running` already hid the launcher: a repeated `running`
    /// for the same game does not hide a window the user brought back.
    hidden_for: HashSet<String>,
    /// The user closed the last window while work was active.
    exit_wanted: bool,
}

fn idle(a: &Activity) -> bool {
    a.running_games == 0 && !a.busy
}

impl WindowPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Is the main window currently destroyed by the policy?
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// Is a quit being held back until the work is done?
    pub fn exit_pending(&self) -> bool {
        self.exit_wanted
    }

    pub fn handle(&mut self, input: PolicyInput, a: &Activity) -> Vec<WindowAction> {
        match input {
            PolicyInput::GameRunning { profile_id } => {
                if a.hide_on_launch {
                    if self.hidden_for.insert(profile_id) && !self.destroyed {
                        self.destroyed = true;
                        return vec![WindowAction::Destroy];
                    }
                    Vec::new()
                } else if !a.keep_launcher_open {
                    vec![WindowAction::Minimize]
                } else {
                    Vec::new()
                }
            }
            PolicyInput::GameEnded { profile_id } => {
                self.hidden_for.remove(&profile_id);
                if self.destroyed {
                    self.destroyed = false;
                    vec![WindowAction::Show]
                } else if self.exit_wanted && idle(a) {
                    self.exit_wanted = false;
                    vec![WindowAction::Exit {
                        apply_update: a.update_ready,
                    }]
                } else {
                    Vec::new()
                }
            }
            PolicyInput::SecondInstance => {
                self.destroyed = false;
                self.exit_wanted = false;
                vec![WindowAction::Show]
            }
            PolicyInput::ExitRequested { programmatic: true } => Vec::new(),
            PolicyInput::ExitRequested {
                programmatic: false,
            } => {
                if self.destroyed {
                    // Our own destroy closed the last window.
                    vec![WindowAction::PreventExit]
                } else if !idle(a) {
                    self.exit_wanted = true;
                    vec![WindowAction::PreventExit]
                } else {
                    vec![WindowAction::Exit {
                        apply_update: a.update_ready,
                    }]
                }
            }
            PolicyInput::Tick => {
                if self.exit_wanted && idle(a) {
                    self.exit_wanted = false;
                    vec![WindowAction::Exit {
                        apply_update: a.update_ready,
                    }]
                } else {
                    Vec::new()
                }
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use WindowAction::*;

    fn act(hide: bool, keep: bool) -> Activity {
        Activity {
            hide_on_launch: hide,
            keep_launcher_open: keep,
            ..Activity::default()
        }
    }
    fn running(id: &str) -> PolicyInput {
        PolicyInput::GameRunning {
            profile_id: id.into(),
        }
    }
    fn ended(id: &str) -> PolicyInput {
        PolicyInput::GameEnded {
            profile_id: id.into(),
        }
    }

    #[test]
    fn hide_on_launch_destroys_on_running_and_recreates_on_exit() {
        let mut p = WindowPolicy::new();
        let mut a = act(true, true);
        a.running_games = 1;
        assert_eq!(p.handle(running("a"), &a), vec![Destroy]);
        assert!(p.is_destroyed());
        // The last window going away must not quit the app.
        assert_eq!(
            p.handle(
                PolicyInput::ExitRequested {
                    programmatic: false
                },
                &a
            ),
            vec![PreventExit]
        );
        a.running_games = 0;
        assert_eq!(p.handle(ended("a"), &a), vec![Show]);
        assert!(!p.is_destroyed());
        // Window is back: closing it now is a normal quit.
        assert_eq!(
            p.handle(
                PolicyInput::ExitRequested {
                    programmatic: false
                },
                &a
            ),
            vec![Exit {
                apply_update: false
            }]
        );
    }

    #[test]
    fn crash_and_stop_both_bring_the_window_back_and_second_instance_too() {
        let mut p = WindowPolicy::new();
        let mut a = act(true, false);
        a.running_games = 1;
        p.handle(running("a"), &a);
        assert_eq!(p.handle(PolicyInput::SecondInstance, &a), vec![Show]);
        assert!(!p.is_destroyed());
        // A repeated `running` of the same game does not hide it again.
        assert!(p.handle(running("a"), &a).is_empty());
        // A different game does.
        a.running_games = 2;
        assert_eq!(p.handle(running("b"), &a), vec![Destroy]);
        assert_eq!(p.handle(ended("b"), &a), vec![Show]);
    }

    #[test]
    fn hide_off_never_destroys() {
        let mut p = WindowPolicy::new();
        let a = act(false, true);
        assert!(p.handle(running("a"), &a).is_empty());
        assert!(!p.is_destroyed());
        assert!(p.handle(ended("a"), &a).is_empty());
    }

    #[test]
    fn keep_launcher_open_off_minimizes() {
        let mut p = WindowPolicy::new();
        assert_eq!(p.handle(running("a"), &act(false, false)), vec![Minimize]);
        // Hide wins over minimize.
        assert_eq!(p.handle(running("b"), &act(true, false)), vec![Destroy]);
    }

    #[test]
    fn closing_the_window_during_a_game_holds_the_quit_until_it_ends() {
        let mut p = WindowPolicy::new();
        let mut a = act(false, true);
        a.running_games = 1;
        assert_eq!(
            p.handle(
                PolicyInput::ExitRequested {
                    programmatic: false
                },
                &a
            ),
            vec![PreventExit]
        );
        assert!(p.exit_pending());
        assert!(p.handle(PolicyInput::Tick, &a).is_empty());
        a.running_games = 0;
        a.update_ready = true;
        assert_eq!(p.handle(ended("a"), &a), vec![Exit { apply_update: true }]);
        assert!(!p.exit_pending());
    }

    #[test]
    fn active_download_holds_the_quit_and_tick_releases_it() {
        let mut p = WindowPolicy::new();
        let mut a = act(false, true);
        a.busy = true;
        assert_eq!(
            p.handle(
                PolicyInput::ExitRequested {
                    programmatic: false
                },
                &a
            ),
            vec![PreventExit]
        );
        assert!(p.handle(PolicyInput::Tick, &a).is_empty());
        a.busy = false;
        assert_eq!(
            p.handle(PolicyInput::Tick, &a),
            vec![Exit {
                apply_update: false
            }]
        );
        assert!(p.handle(PolicyInput::Tick, &a).is_empty());
    }

    #[test]
    fn a_normal_quit_applies_a_staged_update_only_when_one_is_ready() {
        let mut p = WindowPolicy::new();
        let mut a = act(false, true);
        assert_eq!(
            p.handle(
                PolicyInput::ExitRequested {
                    programmatic: false
                },
                &a
            ),
            vec![Exit {
                apply_update: false
            }]
        );
        a.update_ready = true;
        assert_eq!(
            p.handle(
                PolicyInput::ExitRequested {
                    programmatic: false
                },
                &a
            ),
            vec![Exit { apply_update: true }]
        );
    }

    #[test]
    fn programmatic_exit_is_never_blocked() {
        let mut p = WindowPolicy::new();
        let mut a = act(true, true);
        a.running_games = 1;
        p.handle(running("a"), &a);
        assert!(p
            .handle(PolicyInput::ExitRequested { programmatic: true }, &a)
            .is_empty());
    }

    #[test]
    fn second_instance_cancels_a_held_quit() {
        let mut p = WindowPolicy::new();
        let mut a = act(false, true);
        a.busy = true;
        p.handle(
            PolicyInput::ExitRequested {
                programmatic: false,
            },
            &a,
        );
        assert_eq!(p.handle(PolicyInput::SecondInstance, &a), vec![Show]);
        a.busy = false;
        assert!(p.handle(PolicyInput::Tick, &a).is_empty());
    }
}
