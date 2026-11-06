//! The speech bubble.
//!
//! A plugin shows a short remark as a lone status row whose badge is `...`. In
//! the model window that row is drawn as a soft bubble with two droplets trailing
//! toward the cat, instead of the usual compact status panel, and its text wraps
//! so a longer remark grows downward rather than sideways over the artwork.
//!
//! Everything here is pure layout: the bubble's size comes from the font and the
//! window width, so it can be checked without a GPU or a window.

use crate::{HudFont, HudRect};
use bongocat_plugin_sdk::{PluginView, StatusIcon, UiNode};

/// The badge text that marks a status row as a speech bubble.
pub(crate) const BUBBLE_BADGE: &str = "...";

/// The bubble's text size in logical pixels. Smaller than a panel label so a
/// whole remark fits in the empty corner beside the cat's head.
pub(crate) const BUBBLE_TEXT_SIZE: f32 = 10.0;
pub(crate) const BUBBLE_PADDING_X: f32 = 8.0;
pub(crate) const BUBBLE_PADDING_Y: f32 = 6.0;
pub(crate) const BUBBLE_RADIUS: f32 = 12.0;

const BUBBLE_MIN_WIDTH: f32 = 80.0;
const BUBBLE_MAX_WIDTH: f32 = 170.0;

/// How much of the window's width a bubble may take.
pub(crate) const BUBBLE_WIDTH_FRACTION: f32 = 0.32;

const BUBBLE_MAX_LINES: usize = 5;

/// Room below the bubble for its two droplets.
const BUBBLE_DROP_SPACE: f32 = 14.0;

/// A bubble's lines and size, in logical pixels, relative to its own corner.
pub(crate) struct BubbleLayout {
    pub lines: Vec<String>,
    pub line_height: f32,
    /// The rounded body, without the droplets.
    pub body: HudRect,
    /// The whole bubble including the droplets.
    pub width: f32,
    pub height: f32,
}

/// The remark a view carries, if the view is a speech bubble.
pub(crate) fn bubble_text(view: &PluginView) -> Option<&str> {
    match view.nodes.as_slice() {
        [
            UiNode::IconText {
                icon: StatusIcon::Badge(badge),
                text,
            },
        ] if badge.as_str() == BUBBLE_BADGE => Some(text.as_str()),
        _ => None,
    }
}

/// Break `text` into lines no wider than `max_width`, splitting at spaces.
///
/// A single word wider than the limit keeps a line to itself instead of being cut.
pub(crate) fn wrap_text(font: &HudFont, text: &str, size: f32, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
            continue;
        }
        let candidate = format!("{current} {word}");
        if font.measure_text(&candidate, size) <= max_width {
            current = candidate;
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Size a bubble for `text` in a window `viewport_width` logical pixels wide.
pub(crate) fn bubble_layout(font: &HudFont, text: &str, viewport_width: f32) -> BubbleLayout {
    let outer = (viewport_width * BUBBLE_WIDTH_FRACTION).clamp(BUBBLE_MIN_WIDTH, BUBBLE_MAX_WIDTH);
    let text_limit = outer - BUBBLE_PADDING_X * 2.0;
    let mut lines = wrap_text(font, text, BUBBLE_TEXT_SIZE, text_limit);
    lines.truncate(BUBBLE_MAX_LINES);

    let line_height = font.line_height(BUBBLE_TEXT_SIZE);
    let text_width = lines
        .iter()
        .map(|line| font.measure_text(line, BUBBLE_TEXT_SIZE))
        .fold(0.0_f32, f32::max);
    let body_width = text_width + BUBBLE_PADDING_X * 2.0;
    let body_height = lines.len() as f32 * line_height + BUBBLE_PADDING_Y * 2.0;

    BubbleLayout {
        lines,
        line_height,
        body: HudRect::new(0.0, 0.0, body_width, body_height),
        width: body_width,
        height: body_height + BUBBLE_DROP_SPACE,
    }
}

/// The two droplets trailing from the bubble's lower-right corner toward the
/// cat, as (centre x, centre y, radius) in logical pixels.
pub(crate) fn droplets(layout: &BubbleLayout) -> [(f32, f32, f32); 2] {
    let right = layout.body.width;
    let bottom = layout.body.height;
    [
        (right - 14.0, bottom + 4.5, 3.5),
        (right - 8.0, bottom + 10.5, 2.2),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> HudFont {
        crate::load_platform_font().expect("the platform ships a usable text face")
    }

    fn row(badge: &str) -> PluginView {
        PluginView {
            anchor: bongocat_plugin_sdk::Anchor::TopLeft,
            nodes: vec![UiNode::IconText {
                icon: StatusIcon::Badge(badge.into()),
                text: "Hi".into(),
            }],
        }
    }

    #[test]
    fn only_a_lone_dots_badge_row_is_a_bubble() {
        assert_eq!(bubble_text(&row("...")), Some("Hi"));
        assert_eq!(bubble_text(&row("EN")), None);
    }

    #[test]
    fn a_long_remark_wraps_inside_the_width_limit_without_losing_words() {
        let font = font();
        let remark = "Staring at the book doesn't count. I checked.";
        let layout = bubble_layout(&font, remark, 350.0);
        let limit = 350.0 * BUBBLE_WIDTH_FRACTION - BUBBLE_PADDING_X * 2.0;
        assert!(layout.lines.len() > 1, "a long remark takes several lines");
        for line in &layout.lines {
            assert!(font.measure_text(line, BUBBLE_TEXT_SIZE) <= limit + 0.5, "{line}");
        }
        assert_eq!(layout.lines.join(" "), remark);
        assert!(layout.height > layout.body.height, "the droplets have room");
    }
}
