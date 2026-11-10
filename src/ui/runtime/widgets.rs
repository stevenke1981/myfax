//! Leaf widgets: Label, Separator, Button and TextInput.

use acme_core::{Color, DrawCommand};
use acme_layout::LayoutNode;
use acme_style::TextAlign;
use acme_text::{TextStyle, TextWrap};
use acme_textinput::{TextInputState, render_text_input, scroll_caret_into_view};
use acme_theme::Theme;
use acme_widgets::{Button, ButtonState, Label, LabelWrap, TextInput};

use super::edit::{Focus, InputSpec, adopt_value, sync_flags};
use super::geom::{Ctx, FOCUS_RING_WIDTH, Rect4, color, quad, to_rect};
use super::hit::{Region, Target};
use super::paint::Painter;
use super::text::{push_text, text_style};

/// Tolerance so a word-wrapped label re-wraps exactly as its measured layout did.
const WRAP_SLACK: f32 = 0.5;

/// Text style used by inputs; identical to the one `render_text_input` shapes with.
pub(super) fn input_text_style(theme: &Theme) -> TextStyle {
    text_style(
        theme.typography.body,
        theme.typography.body * theme.typography.line_height,
    )
}

impl<M: Clone> Painter<'_, M> {
    pub(super) fn paint_label(&mut self, label: &Label, layout: &LayoutNode, ctx: Ctx) {
        let Some(rect) = self.place(layout, ctx) else {
            return;
        };
        let theme = self.theme;
        let size = label.font_size.unwrap_or(theme.typography.body);
        let line_height = label
            .line_height
            .unwrap_or(theme.typography.body * theme.typography.line_height);
        let style = text_style(size, line_height);
        let text_color = label
            .color
            .or_else(|| {
                label
                    .style
                    .text_color
                    .as_ref()
                    .map(|token| token.resolve(theme))
            })
            .unwrap_or(theme.colors.foreground);
        let width = rect[2].max(0.0);
        let laid = match label.wrap {
            LabelWrap::None => {
                self.text
                    .fit_line(self.fonts, &label.text, &style, width, self.scale)
            }
            LabelWrap::Word => self.text.shape(
                self.fonts,
                &label.text,
                &style,
                Some(width + WRAP_SLACK),
                TextWrap::Word,
                self.scale,
            ),
            LabelWrap::Character => self.text.shape(
                self.fonts,
                &label.text,
                &style,
                Some(width + WRAP_SLACK),
                TextWrap::Glyph,
                self.scale,
            ),
        };
        let x = match label.style.text_align {
            Some(TextAlign::Center) => rect[0] + (rect[2] - laid.width) / 2.0,
            Some(TextAlign::Right) => rect[0] + rect[2] - laid.width,
            _ => rect[0],
        };
        self.out.push(DrawCommand::PushClip(to_rect(rect)));
        push_text(
            self.fonts,
            self.atlas,
            &mut self.out,
            &laid,
            [x, rect[1] + laid.baseline],
            text_color,
        );
        self.out.push(DrawCommand::PopClip);
    }

    pub(super) fn paint_separator(&mut self, layout: &LayoutNode, ctx: Ctx) {
        let Some(rect) = self.place(layout, ctx) else {
            return;
        };
        self.out.push(quad(
            rect,
            color(self.theme.colors.border),
            0.0,
            0.0,
            Color::TRANSPARENT,
        ));
    }

    /// Focus state for a button; records that the focused target was painted.
    fn take_button_focus(&mut self, key: &str, disabled: bool) -> bool {
        let focused = !disabled && self.button_has_focus(key);
        if focused {
            self.focus_found = true;
        }
        focused
    }

    pub(super) fn paint_button(&mut self, button: &Button<M>, layout: &LayoutNode, ctx: Ctx) {
        let Some(rect) = self.place(layout, ctx) else {
            return;
        };
        let theme = self.theme;
        let key = button.key.as_str();
        let focused = self.take_button_focus(key, button.disabled);
        let hovered = matches!(&self.hover, Some(Target::Button(k)) if k == key);
        let pressed = hovered && matches!(&self.pressed, Some(Target::Button(k)) if k == key);
        let resolved = button.resolve_style(
            theme,
            ButtonState {
                hovered,
                pressed,
                focused,
            },
        );
        let ring = focused && self.focus_visible;
        let (border_width, border) = if ring {
            (FOCUS_RING_WIDTH, resolved.focus)
        } else {
            (1.0, resolved.border)
        };
        let radius = button.style.border_radius.unwrap_or(theme.radii.md);
        self.out.push(quad(
            rect,
            color(resolved.background),
            radius,
            border_width,
            color(border),
        ));
        let size = button.style.font_size.unwrap_or(theme.typography.label);
        let line_height = button
            .style
            .line_height
            .unwrap_or(theme.typography.body * theme.typography.line_height);
        let laid = self.text.shape(
            self.fonts,
            &button.label,
            &text_style(size, line_height),
            None,
            TextWrap::None,
            self.scale,
        );
        let x = rect[0] + (rect[2] - laid.width) / 2.0;
        let y = rect[1] + (rect[3] - laid.height) / 2.0 + laid.baseline;
        push_text(
            self.fonts,
            self.atlas,
            &mut self.out,
            &laid,
            [x, y],
            resolved.foreground,
        );
        if !button.disabled {
            self.push_region(Region {
                target: Target::Button(key.to_owned()),
                rect,
                clip: ctx.clip,
                message: button.activate().cloned(),
                input: None,
                blocks: false,
                seq: 0,
            });
        }
    }

    pub(super) fn paint_text_input(&mut self, input: &TextInput<M>, layout: &LayoutNode, ctx: Ctx) {
        let Some(rect) = self.place(layout, ctx) else {
            return;
        };
        let theme = self.theme;
        let key = input.key.as_str();
        let spec = InputSpec {
            value: input.value.clone(),
            password: input.password,
            readonly: input.readonly,
            placeholder: input.placeholder.clone().unwrap_or_default(),
            invalid: input.invalid,
            padding: theme.spacing.px2,
            style: input_text_style(theme),
        };
        let focused_here = self.input_has_focus(key);
        if !input.disabled {
            self.push_region(Region {
                target: Target::Input(key.to_owned()),
                rect,
                clip: ctx.clip,
                message: input.message.clone(),
                input: Some(spec.clone()),
                blocks: false,
                seq: 0,
            });
        }
        if focused_here && !input.disabled {
            self.focus_found = true;
            self.paint_focused_input(&spec, rect);
        } else {
            self.paint_idle_input(&spec, rect);
        }
    }

    fn paint_focused_input(&mut self, spec: &InputSpec, rect: Rect4) {
        let Some(Focus::Input { state, .. }) = self.focus.as_mut() else {
            return;
        };
        sync_flags(state, spec);
        adopt_value(state, &spec.value);
        state.focused = true;
        let content_width = (rect[2] - 2.0 * spec.padding).max(0.0);
        scroll_caret_into_view(state, self.fonts, &spec.style, self.scale, content_width);
        render_text_input(
            &mut self.out,
            state,
            self.fonts,
            self.atlas,
            rect,
            self.theme,
            self.scale,
            true,
            None,
        );
        if self.focus_visible {
            self.out.push(quad(
                rect,
                Color::TRANSPARENT,
                self.theme.radii.sm,
                FOCUS_RING_WIDTH,
                color(self.theme.colors.ring),
            ));
        }
        let caret = state.ime_caret_area(self.fonts, &spec.style, self.scale);
        self.frame.ime_caret = Some([
            rect[0] + spec.padding + caret[0],
            rect[1] + ((rect[3] - caret[3]) / 2.0).max(0.0),
            caret[2],
            caret[3],
        ]);
    }

    fn paint_idle_input(&mut self, spec: &InputSpec, rect: Rect4) {
        let mut state = TextInputState::new();
        sync_flags(&mut state, spec);
        state.text = spec.value.clone();
        render_text_input(
            &mut self.out,
            &mut state,
            self.fonts,
            self.atlas,
            rect,
            self.theme,
            self.scale,
            false,
            None,
        );
    }
}
