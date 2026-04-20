//! Configurable widget layout.
//!
//! The dashboard is described as a recursive tree of
//! **splits** and **widgets**. A split divides its
//! bounds horizontally or vertically among its
//! children; a widget is a leaf that renders into the
//! bounds assigned to it.
//!
//! Children declare their sizing as either a **fixed**
//! pixel count (`size = N`) or a **flex** weight
//! (`flex = N`). On resolution, fixed sizes (plus any
//! divider/gap budget) are subtracted from the parent
//! dimension first; the remainder is shared among flex
//! children in proportion to their weights.
//!
//! See [`Layout::resolve`] for the pure bounds-
//! resolution entry point that underpins the renderer.

use serde::Deserialize;
use thiserror::Error;

/// Top-level layout document. Embedded in the main
/// config under `[dashboard]`, or supplied standalone
/// via `Layout::embedded_default`.
///
/// The TOML shape places `canvas` alongside the root
/// node's fields (via `#[serde(flatten)]`) so the
/// `[dashboard]` section reads without a superfluous
/// `[dashboard.root]` wrapper:
///
/// ```toml
/// [dashboard]
/// canvas = { width = 800, height = 480 }
/// split = "vertical"
/// divider = true
///
/// [[dashboard.children]]
/// size = 50
/// # ...
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    /// Canvas dimensions in pixels.
    pub canvas: Canvas,
    /// Root node of the layout tree (either a split or
    /// a bare widget).
    #[serde(flatten)]
    pub root: Node,
}

/// Canvas dimensions.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Canvas {
    /// Canvas width in pixels.
    pub width: u32,
    /// Canvas height in pixels.
    pub height: u32,
}

/// A node in the layout tree. Either a split container
/// or a leaf widget.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Node {
    /// Container that divides its bounds among children.
    Split(SplitNode),
    /// Leaf widget occupying the node's full bounds.
    Widget(WidgetKind),
}

/// A split container.
#[derive(Debug, Clone, Deserialize)]
pub struct SplitNode {
    /// Split direction.
    pub split: Direction,
    /// Draw a 2-px divider between children. Consumes
    /// 2 px from the split's main axis per gap, and the
    /// resolver emits a [`PlacedDivider`] for each gap
    /// so the renderer can draw the line.
    #[serde(default)]
    pub divider: bool,
    /// Additional whitespace (px) between children.
    /// Applied on top of divider thickness.
    #[serde(default)]
    pub gap: u32,
    /// Ordered list of children; must be non-empty.
    pub children: Vec<Child>,
}

/// Orientation of a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Children stack left → right.
    Horizontal,
    /// Children stack top → bottom.
    Vertical,
}

/// A sized child inside a [`SplitNode`].
///
/// The serde layer accepts either `size = N` (fixed
/// pixels) or `flex = N` (flex weight); supplying
/// neither, both, or `flex = 0` is a deserialization
/// error so invalid states can't enter the tree.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "ChildRaw")]
pub struct Child {
    /// Main-axis sizing policy.
    pub sizing: Sizing,
    /// The child node itself (split or widget).
    pub node: Node,
}

/// Raw serde shape; validated into [`Child`] via
/// `TryFrom` so the `size`/`flex` invariant is enforced
/// at deserialization time rather than at render time.
#[derive(Deserialize)]
struct ChildRaw {
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    flex: Option<u32>,
    #[serde(flatten)]
    node: Node,
}

impl TryFrom<ChildRaw> for Child {
    type Error = &'static str;

    fn try_from(raw: ChildRaw) -> Result<Self, Self::Error> {
        let sizing = match (raw.size, raw.flex) {
            (Some(s), None) => Sizing::Fixed(s),
            (None, Some(0)) => {
                return Err("child `flex` must be at least 1");
            }
            (None, Some(f)) => Sizing::Flex(f),
            (None, None) => {
                return Err("child must have either `size` or `flex`");
            }
            (Some(_), Some(_)) => {
                return Err("child cannot have both `size` and `flex`");
            }
        };
        Ok(Self {
            sizing,
            node: raw.node,
        })
    }
}

impl Child {
    /// Construct a fixed-pixel child. Used by tests and
    /// any programmatic layout builder.
    #[must_use]
    pub fn fixed(size: u32, node: Node) -> Self {
        Self {
            sizing: Sizing::Fixed(size),
            node,
        }
    }

    /// Construct a flex-weighted child. Asserts the
    /// weight is non-zero; use `fixed(0, ...)` if you
    /// literally want a zero-sized slot.
    #[must_use]
    pub fn flex(weight: u32, node: Node) -> Self {
        assert!(weight >= 1, "flex weight must be at least 1");
        Self {
            sizing: Sizing::Flex(weight),
            node,
        }
    }
}

/// Selects which day a weather-domain widget should
/// read from. Parsed from either `day = "today"` or a
/// numeric `day = N` in TOML. String matching is
/// case-insensitive (`"Today"` / `"TODAY"` also work).
///
/// ## Data-source semantics for `Today`
///
/// `DaySelector::Today` resolves to different places in
/// the [`DashboardModel`](crate::dashboard::model::DashboardModel)
/// depending on the widget:
///
/// - [`WidgetKind::WeatherIcon`] and
///   [`WidgetKind::Condition`] read from
///   `model.current.category` (the *now* reading from
///   Home Assistant).
/// - [`WidgetKind::TempHigh`] and [`WidgetKind::TempLow`]
///   read from `model.today.high_c` /
///   `model.today.low_c` (the *day summary* derived
///   from the forecast).
///
/// These two `Option`s are independent: Home Assistant
/// can be reachable while the forecast provider isn't
/// (or the reverse). The split is intentional — "the
/// icon right now" and "today's high" are genuinely
/// different queries — but means a `day = "today"`
/// layout can show, e.g., a live icon next to an
/// em-dash hi/lo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaySelector {
    /// The current reading / today's summary (see the
    /// type-level doc for which widgets read which
    /// `DashboardModel` field).
    Today,
    /// Forecast day at the given offset (0 = first
    /// forecast tile, etc.).
    Offset(u8),
}

impl<'de> Deserialize<'de> for DaySelector {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Str(String),
            Num(u8),
        }
        match Raw::deserialize(d)? {
            Raw::Str(s) if s.eq_ignore_ascii_case("today") => Ok(Self::Today),
            Raw::Str(s) => Err(serde::de::Error::custom(format!(
                "day must be \"today\" or a non-negative integer, got {s:?}"
            ))),
            Raw::Num(n) => Ok(Self::Offset(n)),
        }
    }
}

/// Zero-sized marker for widgets that are meaningful
/// only for `day = "today"` (currently [`WidgetKind::TempNow`]
/// and [`WidgetKind::FeelsLike`]). Accepts the literal
/// `day = "today"` in TOML; rejects numeric offsets at
/// deserialization time with a descriptive error. Also
/// accepts the field being absent via [`Default`], so a
/// terse `widget = "temp-now"` still parses.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TodayOnly;

impl<'de> Deserialize<'de> for TodayOnly {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match DaySelector::deserialize(d)? {
            DaySelector::Today => Ok(Self),
            DaySelector::Offset(n) => Err(serde::de::Error::custom(format!(
                "this widget is today-only; `day = {n}` is not supported. \
                 Use a different widget (e.g. `temp-high`) for forecast days."
            ))),
        }
    }
}

/// Strongly-typed widget enumeration. Every kind the
/// dashboard can render appears here with its
/// parameters. Tagged by the `widget` field in TOML
/// (kebab-cased).
///
/// Weather-domain widgets carry a [`DaySelector`] so a
/// user `layout.toml` can place e.g. the high-temp
/// reading for `day = "today"` next to a forecast
/// day's condition at `day = 1`. No compound "current
/// conditions" / "forecast tile" widgets exist — layouts
/// compose them from atomic widgets.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "widget", rename_all = "kebab-case")]
pub enum WidgetKind {
    /// TRMNL brand label.
    Brand,
    /// Header title text, e.g. "Weather Report".
    HeaderTitle {
        /// Displayed text.
        text: String,
    },
    /// HH:MM wall clock.
    Clock,
    /// Battery outline + percentage.
    Battery,
    /// Weather icon for the selected day.
    WeatherIcon {
        /// Which day's condition to render.
        day: DaySelector,
        /// Icon detail level. `None` (field omitted in
        /// TOML) and `Some(Fidelity::Simple)` both
        /// dispatch through the nine-way coarse icon
        /// table; `Some(Fidelity::Detailed)` prefers a
        /// specialized WMO glyph when a
        /// `WeatherCode::Wmo(_)` is available, falling
        /// back to the coarse icon otherwise. Keeping
        /// the field `Option` lets layout round-trip
        /// losslessly — a diff reviewer can tell
        /// "author opted into simple" from "author
        /// forgot the field".
        #[serde(default)]
        fidelity: Option<Fidelity>,
    },
    /// Big current temperature (now reading). Today-only
    /// because forecast days don't have a single "now"
    /// temperature — use `temp-high` / `temp-low` for
    /// forecast tiles.
    TempNow {
        /// Must be `"today"` (or absent). Enforced at
        /// load time so a layout using `day = 2` fails
        /// with a clear error instead of silently
        /// rendering the current temperature in a
        /// forecast tile.
        #[serde(default)]
        day: TodayOnly,
    },
    /// Condition label for the selected day ("Sunny",
    /// "Partly cloudy", ...).
    Condition {
        /// Which day's condition to label.
        day: DaySelector,
    },
    /// "Feels like N°" line for current conditions.
    /// Today-only: the forecast model has no apparent-
    /// temperature field per day.
    FeelsLike {
        /// Must be `"today"` (or absent). See
        /// [`Self::TempNow`] for rationale.
        #[serde(default)]
        day: TodayOnly,
    },
    /// Weekday abbreviation for the selected day. For
    /// `day = "today"` renders the literal "Today".
    DayName {
        /// Which day's name to render.
        day: DaySelector,
    },
    /// High temperature for the selected day. Renders
    /// as `"N°"` by default, or `"{label} N°"` when
    /// `label` is supplied (`label = "H"` → `"H 12°"`).
    TempHigh {
        /// Which day's high temperature to read.
        day: DaySelector,
        /// Optional text prefix, e.g. `"H"`.
        #[serde(default)]
        label: Option<String>,
    },
    /// Low temperature for the selected day, analogous
    /// to [`Self::TempHigh`].
    TempLow {
        /// Which day's low temperature to read.
        day: DaySelector,
        /// Optional text prefix, e.g. `"L"`.
        #[serde(default)]
        label: Option<String>,
    },
    /// Wind-from-direction + speed cell.
    Wind,
    /// Gust speed cell.
    Gust,
    /// Humidity percentage cell.
    Humidity,
    /// Sunrise time.
    Sunrise,
    /// Sunset time.
    Sunset,
}

/// Icon detail level for [`WidgetKind::WeatherIcon`].
///
/// `Simple` (default) dispatches through the nine-way
/// [`ConditionCategory`](crate::dashboard::classify::ConditionCategory)
/// table — safe for every data source. `Detailed`
/// prefers a WMO-specific glyph when the model carries a
/// recognised code, and silently falls back to the
/// coarse icon otherwise, so the opt-in never produces
/// an empty cell.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fidelity {
    /// Coarse nine-way dispatch keyed on
    /// [`ConditionCategory`](crate::dashboard::classify::ConditionCategory).
    #[default]
    Simple,
    /// Detailed dispatch keyed on
    /// [`WmoCode`](crate::dashboard::classify::WmoCode)
    /// with a coarse fallback.
    Detailed,
}

/// Sizing policy for a [`Child`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sizing {
    /// Fixed-pixel size on the main axis.
    Fixed(u32),
    /// Flex weight share of remaining main-axis space.
    /// Guaranteed `>= 1` by the deserializer.
    Flex(u32),
}

/// Axis-aligned rectangle in canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left edge in pixels.
    pub x: u32,
    /// Top edge in pixels.
    pub y: u32,
    /// Width in pixels.
    pub w: u32,
    /// Height in pixels.
    pub h: u32,
}

/// A widget and its resolved bounds on the canvas.
#[derive(Debug, Clone)]
pub struct PlacedWidget<'a> {
    /// Pixel bounds assigned by the layout resolver.
    pub bounds: Rect,
    /// Widget definition from the layout tree.
    pub widget: &'a WidgetKind,
}

/// A divider line emitted by a split with
/// `divider = true`. Occupies the 2-px gap reserved
/// between adjacent children.
#[derive(Debug, Clone, Copy)]
pub struct PlacedDivider {
    /// Pixel bounds of the 2-px divider strip.
    pub bounds: Rect,
    /// Orientation of the line to draw.
    pub orientation: Direction,
}

/// Output of resolving a [`Layout`] — the flat list of
/// widget placements plus any divider placements.
#[derive(Debug, Clone)]
pub struct Resolved<'a> {
    /// Widget placements in document order.
    pub widgets: Vec<PlacedWidget<'a>>,
    /// Divider placements in document order.
    pub dividers: Vec<PlacedDivider>,
}

/// Errors the layout resolver can report.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LayoutError {
    /// A split has no children.
    #[error("split node must have at least one child")]
    EmptySplit,
    /// Fixed sizes + dividers + gaps exceed the parent
    /// dimension on the split's main axis.
    #[error(
        "fixed sizes ({fixed} px) + separators ({sep} px) \
         exceed available space ({available} px)"
    )]
    Overflow {
        /// Sum of fixed-size children.
        fixed: u32,
        /// Sum of divider + gap budget.
        sep: u32,
        /// Main-axis length of the parent.
        available: u32,
    },
    /// A `u32` arithmetic expression in the resolver
    /// overflowed. Triggered by pathological `size`,
    /// `gap`, or `flex` values in the layout.
    #[error("layout arithmetic overflow")]
    ArithmeticOverflow,
}

impl Layout {
    /// Resolve the layout into a flat list of widget
    /// placements and divider placements covering the
    /// full canvas.
    pub fn resolve(&self) -> Result<Resolved<'_>, LayoutError> {
        let canvas = Rect {
            x: 0,
            y: 0,
            w: self.canvas.width,
            h: self.canvas.height,
        };
        resolve_node(&self.root, canvas)
    }

    /// Embedded default layout (the TOML at
    /// `crates/bellwether/assets/layout.toml`). Used
    /// when the main config has no `[dashboard]` section.
    ///
    /// Parsed **and resolved** once on first use: if
    /// the embedded file ever ceases to parse or
    /// resolve, the first call panics at startup
    /// instead of letting an invalid `Layout`
    /// circulate into the renderer.
    #[must_use]
    pub fn embedded_default() -> &'static Self {
        static LAYOUT: std::sync::OnceLock<Layout> = std::sync::OnceLock::new();
        LAYOUT.get_or_init(|| {
            let src = include_str!("../../../assets/layout.toml");
            let layout: Layout = toml::from_str(src)
                .expect("embedded layout.toml must parse successfully");
            layout
                .resolve()
                .expect("embedded layout.toml must resolve successfully");
            layout
        })
    }
}

/// Resolve an arbitrary node into placements within
/// `bounds`. Primarily used by [`Layout::resolve`];
/// exposed for tests that exercise sub-trees directly.
pub fn resolve_node(
    root: &Node,
    bounds: Rect,
) -> Result<Resolved<'_>, LayoutError> {
    let mut resolved = Resolved {
        widgets: Vec::new(),
        dividers: Vec::new(),
    };
    walk(root, bounds, &mut resolved)?;
    Ok(resolved)
}

fn walk<'a>(
    node: &'a Node,
    bounds: Rect,
    out: &mut Resolved<'a>,
) -> Result<(), LayoutError> {
    match node {
        Node::Widget(widget) => {
            out.widgets.push(PlacedWidget { bounds, widget });
            Ok(())
        }
        Node::Split(split) => walk_split(split, bounds, out),
    }
}

#[allow(clippy::too_many_lines)]
fn walk_split<'a>(
    split: &'a SplitNode,
    bounds: Rect,
    out: &mut Resolved<'a>,
) -> Result<(), LayoutError> {
    if split.children.is_empty() {
        return Err(LayoutError::EmptySplit);
    }

    let axis_len = match split.split {
        Direction::Horizontal => bounds.w,
        Direction::Vertical => bounds.h,
    };

    // Separator budget: 2 px per divider gap + `gap`
    // whitespace between every adjacent pair. All
    // arithmetic is u64 then narrowed so pathological
    // user values can't silently wrap.
    let n_children = u64::try_from(split.children.len())
        .map_err(|_| LayoutError::ArithmeticOverflow)?;
    let gaps = n_children.saturating_sub(1);
    let divider_px: u64 = if split.divider { 2 } else { 0 };
    let sep_per_gap = divider_px
        .checked_add(u64::from(split.gap))
        .ok_or(LayoutError::ArithmeticOverflow)?;
    let sep_total = gaps
        .checked_mul(sep_per_gap)
        .ok_or(LayoutError::ArithmeticOverflow)?;

    let fixed_total: u64 = split
        .children
        .iter()
        .filter_map(|c| match c.sizing {
            Sizing::Fixed(n) => Some(u64::from(n)),
            Sizing::Flex(_) => None,
        })
        .try_fold(0u64, u64::checked_add)
        .ok_or(LayoutError::ArithmeticOverflow)?;

    let reserved = fixed_total
        .checked_add(sep_total)
        .ok_or(LayoutError::ArithmeticOverflow)?;
    if reserved > u64::from(axis_len) {
        return Err(LayoutError::Overflow {
            fixed: u32::try_from(fixed_total).unwrap_or(u32::MAX),
            sep: u32::try_from(sep_total).unwrap_or(u32::MAX),
            available: axis_len,
        });
    }

    let flex_total: u64 = split
        .children
        .iter()
        .filter_map(|c| match c.sizing {
            Sizing::Flex(n) => Some(u64::from(n)),
            Sizing::Fixed(_) => None,
        })
        .try_fold(0u64, u64::checked_add)
        .ok_or(LayoutError::ArithmeticOverflow)?;
    let flex_budget = u64::from(axis_len) - reserved;

    // Resolve each child's main-axis length. Distribute
    // flex budget proportionally; assign any rounding
    // remainder to the last flex child so totals match
    // `flex_budget` exactly.
    let last_flex_idx = split
        .children
        .iter()
        .rposition(|c| matches!(c.sizing, Sizing::Flex(_)));
    let mut lengths: Vec<u32> = Vec::with_capacity(split.children.len());
    let mut flex_assigned: u64 = 0;
    for (idx, child) in split.children.iter().enumerate() {
        let len_u64 = match child.sizing {
            Sizing::Fixed(n) => u64::from(n),
            Sizing::Flex(weight) => {
                if flex_total == 0 {
                    0
                } else if Some(idx) == last_flex_idx {
                    flex_budget - flex_assigned
                } else {
                    let share = flex_budget
                        .checked_mul(u64::from(weight))
                        .ok_or(LayoutError::ArithmeticOverflow)?
                        / flex_total;
                    flex_assigned += share;
                    share
                }
            }
        };
        let len = u32::try_from(len_u64)
            .map_err(|_| LayoutError::ArithmeticOverflow)?;
        lengths.push(len);
    }

    // Walk children, emitting placements and dividers as
    // we advance the cursor along the main axis.
    let sep_per_gap_u32 = u32::try_from(sep_per_gap)
        .map_err(|_| LayoutError::ArithmeticOverflow)?;
    let mut cursor = match split.split {
        Direction::Horizontal => bounds.x,
        Direction::Vertical => bounds.y,
    };
    for (idx, (child, len)) in
        split.children.iter().zip(lengths.iter()).enumerate()
    {
        let child_bounds = match split.split {
            Direction::Horizontal => Rect {
                x: cursor,
                y: bounds.y,
                w: *len,
                h: bounds.h,
            },
            Direction::Vertical => Rect {
                x: bounds.x,
                y: cursor,
                w: bounds.w,
                h: *len,
            },
        };
        walk(&child.node, child_bounds, out)?;
        cursor += len;
        if idx + 1 < split.children.len() {
            if split.divider {
                // The 2-px divider sits at the start of
                // the separator region. Any additional
                // `gap` whitespace follows it.
                let divider_bounds = match split.split {
                    Direction::Horizontal => Rect {
                        x: cursor,
                        y: bounds.y,
                        w: 2,
                        h: bounds.h,
                    },
                    Direction::Vertical => Rect {
                        x: bounds.x,
                        y: cursor,
                        w: bounds.w,
                        h: 2,
                    },
                };
                out.dividers.push(PlacedDivider {
                    bounds: divider_bounds,
                    orientation: split.split,
                });
            }
            cursor += sep_per_gap_u32;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
