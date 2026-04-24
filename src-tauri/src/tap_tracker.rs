//! Platform-agnostic tap detector for a single tracked modifier key.
//!
//! Consumers feed in abstract `Event`s; the tracker returns an `Action`
//! describing what to do with that event (swallow vs pass through) and
//! whether to inject Alt+F3. See
//! `docs/superpowers/specs/2026-04-24-super-tap-start-menu-design.md` for
//! the state machine.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Tracking,
    Tainted,
}

impl Default for State {
    fn default() -> Self {
        State::Idle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The tracked modifier went down. `other_key_held` reflects whether any
    /// non-tracked key (modifier or not) is currently held at press time.
    /// `autorepeat` is true when the platform reports this as a repeat of
    /// a still-held press.
    ModifierDown { other_key_held: bool, autorepeat: bool },
    /// The tracked modifier went up.
    ModifierUp,
    /// Any other key went down or up while we care about it.
    OtherKey,
    /// Passthrough / inhibit was toggled OFF.
    PassthroughOff,
    /// Window focus was lost.
    FocusLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Drop the originating event; don't let the WebView see it.
    Swallow,
    /// Let the event propagate to the WebView normally.
    PassThrough,
    /// Swallow the originating event and inject Alt+F3 into the WebView.
    SwallowAndInjectAltF3,
    /// Tracker-internal signal (passthrough-off / focus-lost); caller had no
    /// event to suppress, so this is a no-op for the event layer.
    Noop,
}

/// Apply an event to the state machine, returning the new state and the
/// action the caller should take.
pub fn step(state: State, event: Event) -> (State, Action) {
    use Action::*;
    use Event::*;
    use State::*;

    match (state, event) {
        // Toggles and focus loss reset regardless of current state.
        (_, PassthroughOff) | (_, FocusLost) => (Idle, Noop),

        // Idle: modifier goes down.
        (Idle, ModifierDown { other_key_held: true, .. }) => (Tainted, Swallow),
        (Idle, ModifierDown { other_key_held: false, .. }) => (Tracking, Swallow),

        // Tracking: autorepeat of our modifier stays in tracking.
        (Tracking, ModifierDown { autorepeat: true, .. }) => (Tracking, Swallow),
        // Tracking: non-autorepeat modifier-down shouldn't happen without a
        // keyup first; treat as taint defensively.
        (Tracking, ModifierDown { .. }) => (Tainted, Swallow),

        // Tracking: another key → tainted.
        (Tracking, OtherKey) => (Tainted, PassThrough),

        // Tracking: clean keyup → this was a tap.
        (Tracking, ModifierUp) => (Idle, SwallowAndInjectAltF3),

        // Tainted: stay tainted until keyup.
        (Tainted, OtherKey) => (Tainted, PassThrough),
        (Tainted, ModifierDown { .. }) => (Tainted, Swallow),
        (Tainted, ModifierUp) => (Idle, Swallow),

        // Idle + stray keyup or other-key: no involvement.
        (Idle, ModifierUp) => (Idle, PassThrough),
        (Idle, OtherKey) => (Idle, PassThrough),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down_clean() -> Event {
        Event::ModifierDown { other_key_held: false, autorepeat: false }
    }

    fn down_with_other_held() -> Event {
        Event::ModifierDown { other_key_held: true, autorepeat: false }
    }

    fn down_repeat() -> Event {
        Event::ModifierDown { other_key_held: false, autorepeat: true }
    }

    #[test]
    fn clean_tap_fires_alt_f3() {
        let (s, a) = step(State::Idle, down_clean());
        assert_eq!((s, a), (State::Tracking, Action::Swallow));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::SwallowAndInjectAltF3));
    }

    #[test]
    fn other_key_while_tracking_taints() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, a) = step(s, Event::OtherKey);
        assert_eq!((s, a), (State::Tainted, Action::PassThrough));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::Swallow));
    }

    #[test]
    fn other_key_already_held_at_down_taints_immediately() {
        let (s, a) = step(State::Idle, down_with_other_held());
        assert_eq!((s, a), (State::Tainted, Action::Swallow));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::Swallow));
    }

    #[test]
    fn autorepeat_modifier_down_stays_in_tracking() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, a) = step(s, down_repeat());
        assert_eq!((s, a), (State::Tracking, Action::Swallow));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::SwallowAndInjectAltF3));
    }

    #[test]
    fn passthrough_off_resets_from_tracking() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, a) = step(s, Event::PassthroughOff);
        assert_eq!((s, a), (State::Idle, Action::Noop));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::PassThrough));
    }

    #[test]
    fn focus_lost_resets_from_tainted() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, _) = step(s, Event::OtherKey);
        let (s, a) = step(s, Event::FocusLost);
        assert_eq!((s, a), (State::Idle, Action::Noop));
    }

    #[test]
    fn idle_other_key_passes_through() {
        let (s, a) = step(State::Idle, Event::OtherKey);
        assert_eq!((s, a), (State::Idle, Action::PassThrough));
    }

    #[test]
    fn double_down_without_up_is_defensively_tainted() {
        let (s, _) = step(State::Idle, down_clean());
        let (s, a) = step(s, down_clean());
        assert_eq!((s, a), (State::Tainted, Action::Swallow));
        let (s, a) = step(s, Event::ModifierUp);
        assert_eq!((s, a), (State::Idle, Action::Swallow));
    }
}
