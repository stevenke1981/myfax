//! Focus, keyboard, IME and pointer editing rules.
//!
//! Text buffers are `acme_textinput::TextInputState`; the cursor is held by that
//! type as a grapheme-aligned byte offset and never leaves this module.

use acme_platform::{Clipboard, PlatformKey};
use acme_text::{FontSystem, TextStyle};
use acme_textinput::{
    TextInputState, byte_offset_to_x, handle_key, handle_keyboard_shortcut, handle_text,
};

use super::hit::{Target, hit_index};
use super::{Modifiers, UiEvent, UiRuntime};

/// The focused widget. Only one input can hold an edit buffer at a time.
pub(super) enum Focus {
    /// Boxed so the enum stays small; the buffer is much larger than `Button`.
    Input {
        key: String,
        state: Box<TextInputState>,
    },
    Button {
        key: String,
    },
}

/// Layout-time description of a text input, captured during paint.
#[derive(Clone, Debug)]
pub(super) struct InputSpec {
    pub value: String,
    pub password: bool,
    pub readonly: bool,
    pub placeholder: String,
    pub invalid: bool,
    /// Inner horizontal padding (`theme.spacing.px2`), used for caret math.
    pub padding: f32,
    pub style: TextStyle,
}

enum Outcome {
    Nothing,
    Edited(String),
    Submit,
}

pub(super) fn new_input_state(spec: &InputSpec) -> TextInputState {
    let mut state = TextInputState::new();
    sync_flags(&mut state, spec);
    state.text = spec.value.clone();
    state.focused = true;
    state.cursor_end();
    state
}

/// Copies widget flags into the buffer; the value is adopted separately.
pub(super) fn sync_flags(state: &mut TextInputState, spec: &InputSpec) {
    state.password = spec.password;
    state.readonly = spec.readonly;
    state.invalid = spec.invalid;
    state.placeholder = spec.placeholder.clone();
}

/// Adopts a value the application changed. The cursor keeps its byte offset,
/// clamped and snapped to the new text.
pub(super) fn adopt_value(state: &mut TextInputState, value: &str) {
    if state.text == value {
        return;
    }
    let cursor = state.cursor();
    state.text = value.to_owned();
    state.clear_history();
    state.move_cursor_to_offset(cursor);
}

/// Char-index cursor for diagnostics; the runtime itself never stores one.
#[cfg(test)]
pub(super) fn char_cursor(state: &TextInputState) -> usize {
    state
        .text
        .get(..state.cursor())
        .map_or(0, |prefix| prefix.chars().count())
}

fn edit_outcome(state: &mut TextInputState, edit: impl FnOnce(&mut TextInputState)) -> Outcome {
    let before = state.text.clone();
    edit(state);
    if state.text == before {
        Outcome::Nothing
    } else {
        Outcome::Edited(state.text.clone())
    }
}

fn input_outcome(
    key: PlatformKey,
    mods: Modifiers,
    text: Option<&str>,
    state: &mut TextInputState,
    clipboard: Option<&Clipboard>,
) -> Outcome {
    match key {
        PlatformKey::Enter if !state.preedit.is_empty() => {
            edit_outcome(state, TextInputState::commit_preedit)
        }
        PlatformKey::Enter => Outcome::Submit,
        PlatformKey::ArrowLeft
        | PlatformKey::ArrowRight
        | PlatformKey::Home
        | PlatformKey::End
        | PlatformKey::Backspace
        | PlatformKey::Delete => edit_outcome(state, |s| {
            handle_key(s, &key, true, None, mods.ctrl, mods.shift);
        }),
        _ if mods.ctrl && !mods.alt => match shortcut(text, mods.shift) {
            Some(letter) => edit_outcome(state, move |s| {
                handle_keyboard_shortcut(s, letter, clipboard);
            }),
            None => Outcome::Nothing,
        },
        _ if mods.ctrl || mods.alt => Outcome::Nothing,
        _ => match typed_text(key, text) {
            Some(typed) => edit_outcome(state, move |s| {
                handle_text(s, &typed);
            }),
            None => Outcome::Nothing,
        },
    }
}

/// Printable text for insertion; control characters are dropped, and Space
/// without text produces a space.
fn typed_text(key: PlatformKey, text: Option<&str>) -> Option<String> {
    let raw = match text {
        Some(value) if !value.is_empty() => value.to_owned(),
        _ if key == PlatformKey::Space => " ".to_owned(),
        _ => return None,
    };
    let filtered: String = raw.chars().filter(|c| !c.is_control()).collect();
    (!filtered.is_empty()).then_some(filtered)
}

/// Maps Ctrl+letter (or the control character the platform reports) to a
/// shortcut name understood by `handle_keyboard_shortcut`.
fn shortcut(text: Option<&str>, shift: bool) -> Option<&'static str> {
    let mut chars = text?.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let letter = match c {
        '\u{1}' => 'a',
        '\u{3}' => 'c',
        '\u{16}' => 'v',
        '\u{18}' => 'x',
        '\u{19}' => 'y',
        '\u{1a}' => 'z',
        c if c.is_ascii_alphabetic() => c.to_ascii_lowercase(),
        _ => return None,
    };
    match (letter, shift) {
        ('z', true) | ('y', _) => Some("y"),
        ('a', _) => Some("a"),
        ('c', _) => Some("c"),
        ('v', _) => Some("v"),
        ('x', _) => Some("x"),
        ('z', _) => Some("z"),
        _ => None,
    }
}

/// Char boundary of `text` nearest to `local_x` (content-local, scroll included).
/// Password fields and non-finite positions fall back to the end.
pub(super) fn boundary_at(
    fonts: &mut FontSystem,
    text: &str,
    style: &TextStyle,
    scale: f32,
    local_x: f32,
    password: bool,
) -> usize {
    if password || text.is_empty() || !local_x.is_finite() {
        return text.len();
    }
    let mut bounds: Vec<usize> = text.char_indices().map(|(index, _)| index).collect();
    bounds.push(text.len());
    let idx =
        bounds.partition_point(|&b| byte_offset_to_x(text, b, &mut *fonts, style, scale) < local_x);
    let Some(&high) = bounds.get(idx) else {
        return text.len();
    };
    let Some(&low) = idx.checked_sub(1).and_then(|i| bounds.get(i)) else {
        return high;
    };
    let low_x = byte_offset_to_x(text, low, &mut *fonts, style, scale);
    let high_x = byte_offset_to_x(text, high, &mut *fonts, style, scale);
    if (local_x - low_x).abs() <= (high_x - local_x).abs() {
        low
    } else {
        high
    }
}

impl<M: Clone + 'static> UiRuntime<M> {
    /// Key press dispatch (releases are filtered by the caller).
    pub(super) fn dispatch_key(
        &mut self,
        key: PlatformKey,
        mods: Modifiers,
        text: Option<&str>,
    ) -> Vec<UiEvent<M>> {
        if key == PlatformKey::Escape {
            return self.escape();
        }
        if key == PlatformKey::Tab {
            self.cycle_focus(mods.shift);
            return Vec::new();
        }
        if matches!(self.focus, Some(Focus::Input { .. })) {
            return self.input_key(key, mods, text);
        }
        if matches!(self.focus, Some(Focus::Button { .. }))
            && matches!(key, PlatformKey::Enter | PlatformKey::Space)
        {
            return self.activate_focused_button();
        }
        Vec::new()
    }

    /// Escape cancels an IME composition first; otherwise it blurs and emits `Cancel`.
    fn escape(&mut self) -> Vec<UiEvent<M>> {
        if let Some(Focus::Input { state, .. }) = self.focus.as_mut()
            && !state.preedit.is_empty()
        {
            state.cancel_preedit();
            return Vec::new();
        }
        self.focus = None;
        vec![UiEvent::Cancel]
    }

    fn input_key(
        &mut self,
        key: PlatformKey,
        mods: Modifiers,
        text: Option<&str>,
    ) -> Vec<UiEvent<M>> {
        if mods.ctrl {
            self.ensure_clipboard();
        }
        let clipboard = self.clipboard.as_ref();
        let Some(Focus::Input {
            key: input_key,
            state,
        }) = self.focus.as_mut()
        else {
            return Vec::new();
        };
        let input_key = input_key.clone();
        match input_outcome(key, mods, text, state, clipboard) {
            Outcome::Nothing => Vec::new(),
            Outcome::Edited(value) => vec![UiEvent::TextChanged {
                key: input_key,
                value,
            }],
            Outcome::Submit => vec![UiEvent::Submit { key: input_key }],
        }
    }

    fn activate_focused_button(&self) -> Vec<UiEvent<M>> {
        let Some(Focus::Button { key }) = &self.focus else {
            return Vec::new();
        };
        self.frame
            .regions
            .iter()
            .find(|region| matches!(&region.target, Target::Button(k) if k == key))
            .and_then(|region| region.message.clone())
            .map(UiEvent::Message)
            .into_iter()
            .collect()
    }

    pub(super) fn focus_target(&self) -> Option<Target> {
        match &self.focus {
            Some(Focus::Input { key, .. }) => Some(Target::Input(key.clone())),
            Some(Focus::Button { key }) => Some(Target::Button(key.clone())),
            None => None,
        }
    }

    /// Tab order: focusable regions in paint order, restricted to the last
    /// `modal:` container's paint span while one is open.
    fn focus_order(&self) -> Vec<usize> {
        let span = self.frame.modal;
        self.frame
            .regions
            .iter()
            .enumerate()
            .filter(|(_, region)| matches!(region.target, Target::Button(_) | Target::Input(_)))
            .filter(|(_, region)| {
                span.is_none_or(|(start, end)| region.seq > start && region.seq < end)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn cycle_focus(&mut self, backwards: bool) {
        let order = self.focus_order();
        let Some(last) = order.len().checked_sub(1) else {
            self.focus = None;
            return;
        };
        let current = self.focus_target();
        let position = current.as_ref().and_then(|wanted| {
            order.iter().position(|&index| {
                self.frame
                    .regions
                    .get(index)
                    .is_some_and(|region| region.target == *wanted)
            })
        });
        let next = match (position, backwards) {
            (Some(0), true) | (None, true) => last,
            (Some(p), true) => p - 1,
            (Some(p), false) if p == last => 0,
            (Some(p), false) => p + 1,
            (None, false) => 0,
        };
        let Some(&region_index) = order.get(next) else {
            return;
        };
        let Some(target) = self
            .frame
            .regions
            .get(region_index)
            .map(|r| r.target.clone())
        else {
            return;
        };
        if Some(&target) == current.as_ref() {
            return;
        }
        match target {
            Target::Button(key) => self.focus = Some(Focus::Button { key }),
            Target::Input(_) => self.focus_input_region(region_index, None),
            Target::Container(_) => {}
        }
    }

    /// Focuses the input at `index`. An already-focused input keeps its buffer;
    /// `click_x` moves the caret to the clicked character.
    fn focus_input_region(&mut self, index: usize, click_x: Option<f32>) {
        let Some(region) = self.frame.regions.get(index) else {
            return;
        };
        let (Target::Input(key), Some(spec)) = (region.target.clone(), region.input.clone()) else {
            return;
        };
        let text_x = region.rect[0] + spec.padding;
        let scale = self.scale;
        if let Some(Focus::Input {
            key: current,
            state,
        }) = self.focus.as_mut()
            && *current == key
        {
            if let Some(x) = click_x {
                let text = state.text.clone();
                let local = x - text_x + state.scroll_offset;
                let cursor = boundary_at(
                    &mut self.fonts,
                    &text,
                    &spec.style,
                    scale,
                    local,
                    spec.password,
                );
                state.move_cursor_to_offset(cursor);
            }
            return;
        }
        let mut state = new_input_state(&spec);
        if let Some(x) = click_x {
            let cursor = boundary_at(
                &mut self.fonts,
                &spec.value,
                &spec.style,
                scale,
                x - text_x,
                spec.password,
            );
            state.move_cursor_to_offset(cursor);
        }
        self.focus = Some(Focus::Input {
            key,
            state: Box::new(state),
        });
    }

    pub(super) fn pointer_down(&mut self, x: f32, y: f32) {
        let target = hit_index(&self.frame.regions, x, y)
            .and_then(|index| self.frame.regions.get(index))
            .map(|region| region.target.clone());
        let focused = self.focus_target();
        if matches!(focused, Some(Target::Input(_))) && focused != target {
            self.focus = None;
        }
        self.pressed = target;
    }

    pub(super) fn pointer_up(&mut self, x: f32, y: f32) -> Vec<UiEvent<M>> {
        let pressed = self.pressed.take();
        let Some(index) = hit_index(&self.frame.regions, x, y) else {
            return Vec::new();
        };
        let Some(region) = self.frame.regions.get(index) else {
            return Vec::new();
        };
        if pressed.as_ref() != Some(&region.target) {
            return Vec::new();
        }
        let message = region.message.clone();
        match region.target.clone() {
            Target::Button(key) => self.focus = Some(Focus::Button { key }),
            Target::Input(_) => self.focus_input_region(index, Some(x)),
            Target::Container(_) => self.focus = None,
        }
        message.map(UiEvent::Message).into_iter().collect()
    }

    /// Returns `true` when the composition text changed.
    pub(super) fn ime_preedit_text(&mut self, text: &str) -> bool {
        let Some(Focus::Input { state, .. }) = self.focus.as_mut() else {
            return false;
        };
        if state.preedit == text {
            return false;
        }
        if text.is_empty() {
            state.cancel_preedit();
        } else {
            state.set_preedit(text, None);
        }
        true
    }

    pub(super) fn ime_commit_text(&mut self, text: &str) -> Vec<UiEvent<M>> {
        let Some(Focus::Input { key, state }) = self.focus.as_mut() else {
            return Vec::new();
        };
        let key = key.clone();
        let outcome = edit_outcome(state, |s| {
            handle_text(s, text);
        });
        state.cancel_preedit();
        match outcome {
            Outcome::Edited(value) => vec![UiEvent::TextChanged { key, value }],
            Outcome::Nothing | Outcome::Submit => Vec::new(),
        }
    }

    /// Creates the clipboard once; on failure copy/paste become no-ops.
    fn ensure_clipboard(&mut self) {
        if self.clipboard_tried {
            return;
        }
        self.clipboard_tried = true;
        self.clipboard = match Clipboard::new() {
            Ok(clipboard) => Some(clipboard),
            Err(error) => {
                tracing::debug!(%error, "clipboard unavailable; copy and paste disabled");
                None
            }
        };
    }
}
