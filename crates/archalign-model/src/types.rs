//! Strict serde types for Archify JSON v1 architecture diagrams (ADR 0012).
//!
//! Every property of the vendored `architecture.schema.json` and `common.schema.json` is
//! represented. Every struct denies unknown fields, mirroring `additionalProperties: false`.
//! Optional arrays stay `Option<Vec<_>>` so an empty array and a missing one round-trip
//! differently.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::num::{Num, Pair};

/// A whole Archify architecture document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchitectureDoc {
    /// Always `1`.
    pub schema_version: u64,
    /// Always `"architecture"`.
    pub diagram_type: String,
    /// Title, output path, presentation and provenance metadata.
    pub meta: Meta,
    /// Grid placement settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<GridLayout>,
    /// The boxes.
    pub components: Vec<Component>,
    /// Labelled rectangles around groups of components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boundaries: Option<Vec<Boundary>>,
    /// Directed relationships.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connections: Option<Vec<Connection>>,
    /// Summary cards.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cards: Option<Vec<Card>>,
}

impl ArchitectureDoc {
    /// Boundaries, or an empty slice.
    pub fn boundaries(&self) -> &[Boundary] {
        self.boundaries.as_deref().unwrap_or(&[])
    }

    /// Connections, or an empty slice.
    pub fn connections(&self) -> &[Connection] {
        self.connections.as_deref().unwrap_or(&[])
    }

    /// Cards, or an empty slice.
    pub fn cards(&self) -> &[Card] {
        self.cards.as_deref().unwrap_or(&[])
    }

    /// The component with this ID.
    pub fn component(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }
}

/// `meta`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Meta {
    /// Diagram title.
    pub title: String,
    /// BCP 47-ish locale tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Translation strings (parsed and preserved; not used in v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translations: Option<BTreeMap<String, String>>,
    /// Subtitle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Portable relative HTML output path.
    pub output: String,
    /// Animation mode (preserved; not used in v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<Animation>,
    /// Visual preset (preserved; not used in v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visual_preset: Option<VisualPreset>,
    /// Quality profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_profile: Option<QualityProfile>,
    /// Engineering profile (preserved; not used in v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engineering_profile: Option<EngineeringProfile>,
    /// Source repository provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<Repository>,
    /// Guided views (preserved; not used in v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<Vec<GuidedView>>,
    /// Legend settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<Legend>,
    /// Authored canvas size.
    #[serde(rename = "viewBox", default, skip_serializing_if = "Option::is_none")]
    pub view_box: Option<Pair>,
}

/// `common#/$defs/animation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Animation {
    /// Trace animation.
    Trace,
    /// No animation.
    None,
}

/// `common#/$defs/visualPreset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VisualPreset {
    /// `classic`
    Classic,
    /// `signal-flow`
    SignalFlow,
    /// `blueprint`
    Blueprint,
    /// `editorial`
    Editorial,
}

/// `common#/$defs/qualityProfile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QualityProfile {
    /// `standard`
    Standard,
    /// `showcase`
    Showcase,
}

/// `meta.engineering_profile`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineeringProfile {
    /// `deployment-ownership`
    DeploymentOwnership,
}

/// `common#/$defs/repository`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repository {
    /// Repository URL (credential-free).
    pub url: String,
    /// Hosting provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<RepositoryProvider>,
    /// Whether citations link to the web or stay local.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_mode: Option<LinkMode>,
    /// 40-hex commit.
    pub revision: String,
}

/// `repository.provider`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepositoryProvider {
    /// `github`
    Github,
    /// `gitee`
    Gitee,
}

/// `repository.link_mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkMode {
    /// `web`
    Web,
    /// `local-only`
    LocalOnly,
}

/// One entry of `common#/$defs/guidedViews`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuidedView {
    /// View ID.
    pub id: String,
    /// View label.
    pub label: String,
    /// Focused component IDs.
    pub focus: Vec<String>,
    /// Optional note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// `meta.legend`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Legend {
    /// `auto`, `all` or `hidden`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<LegendMode>,
    /// Per-type overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entries: Option<LegendEntries>,
}

/// `common#/$defs/legendMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegendMode {
    /// Show the types present.
    #[default]
    Auto,
    /// Show all seven types.
    All,
    /// Show no legend.
    Hidden,
}

/// `meta.legend.entries`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegendEntries {
    /// `frontend`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontend: Option<LegendEntry>,
    /// `backend`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<LegendEntry>,
    /// `database`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<LegendEntry>,
    /// `cloud`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cloud: Option<LegendEntry>,
    /// `security`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<LegendEntry>,
    /// `messagebus`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messagebus: Option<LegendEntry>,
    /// `external`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<LegendEntry>,
}

impl LegendEntries {
    /// The entry for a component type.
    pub fn get(&self, ty: ComponentType) -> Option<&LegendEntry> {
        match ty {
            ComponentType::Frontend => self.frontend.as_ref(),
            ComponentType::Backend => self.backend.as_ref(),
            ComponentType::Database => self.database.as_ref(),
            ComponentType::Cloud => self.cloud.as_ref(),
            ComponentType::Security => self.security.as_ref(),
            ComponentType::Messagebus => self.messagebus.as_ref(),
            ComponentType::External => self.external.as_ref(),
        }
    }
}

/// `common#/$defs/legendEntry`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegendEntry {
    /// Replacement text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// `false` hides the entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

/// Top-level `layout` (grid mode).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GridLayout {
    /// Always `grid`.
    pub mode: LayoutMode,
    /// Grid origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<Pair>,
    /// Column count (1–12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u64>,
    /// Horizontal gap.
    #[serde(rename = "gapX", default, skip_serializing_if = "Option::is_none")]
    pub gap_x: Option<Num>,
    /// Vertical gap.
    #[serde(rename = "gapY", default, skip_serializing_if = "Option::is_none")]
    pub gap_y: Option<Num>,
    /// Cell width.
    #[serde(rename = "cellW", default, skip_serializing_if = "Option::is_none")]
    pub cell_w: Option<Num>,
    /// Cell height.
    #[serde(rename = "cellH", default, skip_serializing_if = "Option::is_none")]
    pub cell_h: Option<Num>,
}

/// `layout.mode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    /// `grid`
    Grid,
}

/// `common#/$defs/componentType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentType {
    /// `frontend`
    Frontend,
    /// `backend`
    Backend,
    /// `database`
    Database,
    /// `cloud`
    Cloud,
    /// `security`
    Security,
    /// `messagebus`
    Messagebus,
    /// `external`
    External,
}

impl ComponentType {
    /// All seven types, in legend order.
    pub const ALL: [ComponentType; 7] = [
        ComponentType::Frontend,
        ComponentType::Backend,
        ComponentType::Database,
        ComponentType::Cloud,
        ComponentType::Security,
        ComponentType::Messagebus,
        ComponentType::External,
    ];

    /// The schema name.
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentType::Frontend => "frontend",
            ComponentType::Backend => "backend",
            ComponentType::Database => "database",
            ComponentType::Cloud => "cloud",
            ComponentType::Security => "security",
            ComponentType::Messagebus => "messagebus",
            ComponentType::External => "external",
        }
    }

    /// Default legend text (Archify's English labels).
    pub fn default_label(self) -> &'static str {
        match self {
            ComponentType::Frontend => "Frontend",
            ComponentType::Backend => "Backend",
            ComponentType::Database => "Database",
            ComponentType::Cloud => "Cloud",
            ComponentType::Security => "Security",
            ComponentType::Messagebus => "Message bus",
            ComponentType::External => "External",
        }
    }
}

/// `common#/$defs/nodeIcon` (preserved; not drawn in v1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum NodeIcon {
    Calendar,
    Clock,
    Person,
    Briefcase,
    Flag,
    Moon,
    Frontend,
    Backend,
    Database,
    Cloud,
    Security,
    Messagebus,
    External,
    Start,
    Active,
    Waiting,
    Success,
    Failure,
    Neutral,
    None,
}

/// `common#/$defs/brandMark` (preserved; not drawn in v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BrandMark {
    /// A built-in mark name or an `http(s)` URL.
    Name(String),
    /// A pinned remote asset.
    Asset(BrandAsset),
}

/// The object form of a brand mark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrandAsset {
    /// Asset URL.
    pub url: String,
    /// Lowercase hex SHA-256 of the asset.
    pub sha256: String,
}

/// One entry of `components[]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// Unique ID.
    pub id: String,
    /// Component type (drives color and legend).
    #[serde(rename = "type")]
    pub ty: ComponentType,
    /// Main label.
    pub label: String,
    /// Secondary label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sublabel: Option<String>,
    /// Small tag text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Corner icon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<NodeIcon>,
    /// Brand mark.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand: Option<BrandMark>,
    /// 1–3 citations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<SourceRef>>,
    /// Grid row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<u64>,
    /// Grid column.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub col: Option<u64>,
    /// Top-left position.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<Pair>,
    /// Width and height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Pair>,
}

impl Component {
    /// Citations, or an empty slice.
    pub fn sources(&self) -> &[SourceRef] {
        self.sources.as_deref().unwrap_or(&[])
    }
}

/// One entry of `common#/$defs/sourceReferences`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    /// Repo-relative path.
    pub path: String,
    /// First cited line (1-based).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// Last cited line (inclusive).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u64>,
    /// Short description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// `boundaries[].kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoundaryKind {
    /// `region`
    Region,
    /// `security-group`
    SecurityGroup,
}

/// One entry of `boundaries[]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundary {
    /// Region or security group.
    pub kind: BoundaryKind,
    /// Label drawn at the top-left.
    pub label: String,
    /// Member component IDs.
    pub wraps: Vec<String>,
    /// Uniform padding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pad: Option<Num>,
}

/// `common#/$defs/variant`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    /// `default`
    #[default]
    Default,
    /// `emphasis`
    Emphasis,
    /// `security`
    Security,
    /// `dashed`
    Dashed,
}

impl Variant {
    /// The schema name.
    pub fn as_str(self) -> &'static str {
        match self {
            Variant::Default => "default",
            Variant::Emphasis => "emphasis",
            Variant::Security => "security",
            Variant::Dashed => "dashed",
        }
    }
}

/// `common#/$defs/side`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// `left`
    Left,
    /// `right`
    Right,
    /// `top`
    Top,
    /// `bottom`
    Bottom,
}

/// `connections[].route`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// `auto`
    Auto,
    /// `straight`
    Straight,
    /// `orthogonal-h`
    OrthogonalH,
    /// `orthogonal-v`
    OrthogonalV,
}

/// One entry of `connections[]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    /// Optional unique ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Source component.
    pub from: String,
    /// Target component.
    pub to: String,
    /// Edge label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Line style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<Variant>,
    /// Side of `from` the edge leaves.
    #[serde(rename = "fromSide", default, skip_serializing_if = "Option::is_none")]
    pub from_side: Option<Side>,
    /// Side of `to` the edge enters.
    #[serde(rename = "toSide", default, skip_serializing_if = "Option::is_none")]
    pub to_side: Option<Side>,
    /// Routing hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    /// Waypoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<Vec<Pair>>,
    /// Absolute label position.
    #[serde(rename = "labelAt", default, skip_serializing_if = "Option::is_none")]
    pub label_at: Option<Pair>,
    /// Label x offset.
    #[serde(rename = "labelDx", default, skip_serializing_if = "Option::is_none")]
    pub label_dx: Option<Num>,
    /// Label y offset.
    #[serde(rename = "labelDy", default, skip_serializing_if = "Option::is_none")]
    pub label_dy: Option<Num>,
    /// Segment index carrying the label.
    #[serde(
        rename = "labelSegment",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub label_segment: Option<u64>,
    /// Stroke width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<Num>,
}

impl Connection {
    /// The variant, defaulting to `default`.
    pub fn variant(&self) -> Variant {
        self.variant.unwrap_or_default()
    }
}

/// `common#/$defs/cards[].dot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)]
pub enum CardDot {
    Cyan,
    Emerald,
    Violet,
    Amber,
    Rose,
    Orange,
    Slate,
}

/// One entry of `cards[]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    /// Dot color.
    pub dot: CardDot,
    /// Card title.
    pub title: String,
    /// Bullet items.
    pub items: Vec<String>,
}
