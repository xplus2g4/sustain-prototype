//! The bundled JetBrains Mono font and text measurement (ADR 0010).

use std::borrow::Cow;
use std::sync::Once;

use archalign_layout::{Size, TextMeasure};
use iced::advanced::graphics::text::{Paragraph, font_system};
use iced::advanced::text::{Paragraph as _, Shaping, Wrapping};
use iced::{Font, Pixels, alignment, font};

/// JetBrains Mono Regular 2.304 (OFL-1.1).
pub const REGULAR_BYTES: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
/// JetBrains Mono Bold 2.304 (OFL-1.1).
pub const BOLD_BYTES: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");

/// The family used everywhere.
pub const MONO: Font = Font::with_name("JetBrains Mono");
/// Bold variant, for node labels and headings.
pub const MONO_BOLD: Font = Font {
    weight: font::Weight::Bold,
    ..MONO
};

/// Loads the bundled fonts into iced's global font system once. Needed wherever text is
/// measured or drawn outside the running application (worker threads, tests).
pub fn ensure_loaded() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        if let Ok(mut system) = font_system().write() {
            system.load_font(Cow::Borrowed(REGULAR_BYTES));
            system.load_font(Cow::Borrowed(BOLD_BYTES));
        }
    });
}

/// Measures text with iced's paragraph shaping in JetBrains Mono. Uses the global font
/// system, so it needs no renderer and works on any thread.
#[derive(Debug, Clone, Copy, Default)]
pub struct IcedTextMeasure;

impl TextMeasure for IcedTextMeasure {
    fn measure(&self, text: &str, size_px: f64) -> Size {
        ensure_loaded();
        let paragraph = Paragraph::with_text(iced::advanced::Text {
            content: text,
            bounds: iced::Size::INFINITE,
            size: Pixels(size_px as f32),
            line_height: iced::widget::text::LineHeight::default(),
            font: MONO,
            align_x: iced::advanced::text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: Shaping::Advanced,
            wrapping: Wrapping::None,
        });
        let b = paragraph.min_bounds();
        Size::new(f64::from(b.width), f64::from(b.height))
    }
}
