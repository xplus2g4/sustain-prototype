//! Archify's dark and light colors (from `archify/assets/template.html`, MIT; ADR 0010).

use archalign_model::{BoundaryKind, CardDot, ComponentType, Variant};
use iced::Color;

/// Light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    /// Archify's default dark theme.
    #[default]
    Dark,
    /// Archify's light theme.
    Light,
}

impl ThemeChoice {
    /// The other theme.
    pub fn toggled(self) -> Self {
        match self {
            ThemeChoice::Dark => ThemeChoice::Light,
            ThemeChoice::Light => ThemeChoice::Dark,
        }
    }

    /// The palette for this theme.
    pub fn palette(self) -> &'static Palette {
        match self {
            ThemeChoice::Dark => &DARK,
            ThemeChoice::Light => &LIGHT,
        }
    }
}

/// Fill and stroke of one component type.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeColors {
    /// Box fill.
    pub fill: Color,
    /// Box border and accents.
    pub stroke: Color,
}

/// Every color the app chrome and the diagram use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// `--bg`
    pub bg: Color,
    /// `--grid`
    pub grid: Color,
    /// `--text`
    pub text: Color,
    /// `--text-muted`
    pub text_muted: Color,
    /// `--text-dim`
    pub text_dim: Color,
    /// `--panel`
    pub panel: Color,
    /// `--panel-border`
    pub panel_border: Color,
    /// `--arrow`
    pub arrow: Color,
    /// `--arrow-emphasis`
    pub arrow_emphasis: Color,
    /// `--mask` (opaque backing behind translucent fills)
    pub mask: Color,
    /// `--toolbar-bg`
    pub toolbar_bg: Color,
    /// `--toolbar-border`
    pub toolbar_border: Color,
    /// `--toolbar-text`
    pub toolbar_text: Color,
    /// Component types, in [`ComponentType::ALL`] order.
    pub types: [TypeColors; 7],
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::from_rgba8(r, g, b, a)
}

/// `:root, [data-theme="dark"]`.
pub static DARK: Palette = Palette {
    bg: rgb(0x020617),
    grid: rgb(0x1e293b),
    text: rgb(0xffffff),
    text_muted: rgb(0x94a3b8),
    text_dim: rgb(0x475569),
    panel: rgba(15, 23, 42, 0.5),
    panel_border: rgb(0x1e293b),
    arrow: rgb(0x64748b),
    arrow_emphasis: rgb(0x34d399),
    mask: rgb(0x0f172a),
    toolbar_bg: rgba(15, 23, 42, 0.8),
    toolbar_border: rgb(0x334155),
    toolbar_text: rgb(0xe2e8f0),
    types: [
        TypeColors {
            fill: rgba(8, 51, 68, 0.4),
            stroke: rgb(0x22d3ee),
        },
        TypeColors {
            fill: rgba(6, 78, 59, 0.4),
            stroke: rgb(0x34d399),
        },
        TypeColors {
            fill: rgba(76, 29, 149, 0.4),
            stroke: rgb(0xa78bfa),
        },
        TypeColors {
            fill: rgba(120, 53, 15, 0.3),
            stroke: rgb(0xfbbf24),
        },
        TypeColors {
            fill: rgba(136, 19, 55, 0.4),
            stroke: rgb(0xfb7185),
        },
        TypeColors {
            fill: rgba(251, 146, 60, 0.3),
            stroke: rgb(0xfb923c),
        },
        TypeColors {
            fill: rgba(30, 41, 59, 0.5),
            stroke: rgb(0x94a3b8),
        },
    ],
};

/// `[data-theme="light"]`.
pub static LIGHT: Palette = Palette {
    bg: rgb(0xf4f5f7),
    grid: rgb(0xe9edf2),
    text: rgb(0x111827),
    text_muted: rgb(0x5b6474),
    text_dim: rgb(0x9aa3b2),
    panel: rgb(0xffffff),
    panel_border: rgb(0xe5e7eb),
    arrow: rgb(0x94a3b8),
    arrow_emphasis: rgb(0x059669),
    mask: rgb(0xffffff),
    toolbar_bg: rgba(255, 255, 255, 0.92),
    toolbar_border: rgb(0xcbd5e1),
    toolbar_text: rgb(0x334155),
    types: [
        TypeColors {
            fill: rgba(34, 211, 238, 0.15),
            stroke: rgb(0x0891b2),
        },
        TypeColors {
            fill: rgba(52, 211, 153, 0.18),
            stroke: rgb(0x059669),
        },
        TypeColors {
            fill: rgba(167, 139, 250, 0.2),
            stroke: rgb(0x7c3aed),
        },
        TypeColors {
            fill: rgba(251, 191, 36, 0.18),
            stroke: rgb(0xd97706),
        },
        TypeColors {
            fill: rgba(251, 113, 133, 0.15),
            stroke: rgb(0xe11d48),
        },
        TypeColors {
            fill: rgba(251, 146, 60, 0.15),
            stroke: rgb(0xea580c),
        },
        TypeColors {
            fill: rgba(148, 163, 184, 0.18),
            stroke: rgb(0x64748b),
        },
    ],
};

impl Palette {
    /// Colors of a component type.
    pub fn component(&self, ty: ComponentType) -> TypeColors {
        let i = ComponentType::ALL
            .iter()
            .position(|t| *t == ty)
            .unwrap_or(6);
        self.types[i]
    }

    /// Stroke color and dash pattern of a connection variant (`.a-*` classes).
    pub fn connection(&self, variant: Variant) -> (Color, Option<[f32; 2]>) {
        match variant {
            Variant::Default => (self.arrow, None),
            Variant::Emphasis => (self.arrow_emphasis, None),
            Variant::Security => (
                self.component(ComponentType::Security).stroke,
                Some([5.0, 5.0]),
            ),
            Variant::Dashed => (
                self.component(ComponentType::Database).stroke,
                Some([4.0, 4.0]),
            ),
        }
    }

    /// Fill, stroke and dash pattern of a boundary (`.c-region`, `.c-security-group`).
    pub fn boundary(&self, kind: BoundaryKind) -> (Color, Color, [f32; 2]) {
        match kind {
            BoundaryKind::Region => (
                rgba(251, 191, 36, 0.05),
                self.component(ComponentType::Cloud).stroke,
                [8.0, 4.0],
            ),
            BoundaryKind::SecurityGroup => (
                Color::TRANSPARENT,
                self.component(ComponentType::Security).stroke,
                [4.0, 4.0],
            ),
        }
    }

    /// Card dot color (`.card-dot.*`).
    pub fn card_dot(&self, dot: CardDot) -> Color {
        let ty = match dot {
            CardDot::Cyan => ComponentType::Frontend,
            CardDot::Emerald => ComponentType::Backend,
            CardDot::Violet => ComponentType::Database,
            CardDot::Amber => ComponentType::Cloud,
            CardDot::Rose => ComponentType::Security,
            CardDot::Orange => ComponentType::Messagebus,
            CardDot::Slate => ComponentType::External,
        };
        self.component(ty).stroke
    }

    /// Accent used for selection and reverse highlights.
    pub fn accent(&self) -> Color {
        self.arrow_emphasis
    }

    /// Warning color (badges, banners).
    pub fn warning(&self) -> Color {
        self.component(ComponentType::Cloud).stroke
    }

    /// Error color (banners).
    pub fn danger(&self) -> Color {
        self.component(ComponentType::Security).stroke
    }

    /// The iced theme for the app chrome.
    pub fn iced_theme(&self, name: &'static str) -> iced::Theme {
        iced::Theme::custom(
            name,
            iced::theme::Palette {
                background: self.bg,
                text: self.text,
                primary: self.component(ComponentType::Frontend).stroke,
                success: self.arrow_emphasis,
                warning: self.warning(),
                danger: self.danger(),
            },
        )
    }
}
