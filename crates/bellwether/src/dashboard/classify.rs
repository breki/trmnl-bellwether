//! Weather classification + compass-direction bucketing.
//!
//! Pure functions that translate already-normalised
//! forecast values (°C, km/h, mm, compass degrees) into
//! the display-domain enums the dashboard consumes.
//! Unit conversion and u/v → (magnitude, direction)
//! arithmetic live in the provider adapter (see
//! `crate::clients::windy::snapshot`); this module is
//! pure display-layer logic.
//!
//! ## Two-tier weather taxonomy
//!
//! - [`WmoCode`] — the full WMO 4677 code list (28
//!   variants). Sourced from the provider's
//!   `weather_code` field and narrowed via
//!   [`WmoCode::try_from`] at the system boundary.
//! - [`ConditionCategory`] — a 9-variant coarse view
//!   computed on demand via [`WmoCode::coarsen`]. Never
//!   stored — the moment a caller needs "a broad
//!   bucket", it asks for it, so there's no way for
//!   the two to drift.
//!
//! [`classify_category`] is the entry point the
//! display layer should use once migrated: it prefers
//! the provider's `weather_code` and only falls back
//! to cloud+precip when the provider has no code for
//! that hour. Input comes in as [`WeatherCode`], which
//! preserves the distinction between "provider sent
//! nothing" (`None`) and "provider sent a code we
//! don't recognise" (`Some(WeatherCode::Unrecognised(n))`).

use thiserror::Error;

/// Qualitative weather state the dashboard icons and
/// labels select from. The ordering is "nicer weather
/// first" purely for source-reading convenience; no
/// code relies on the discriminant values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    /// Clear / mostly-clear skies, no precipitation.
    Sunny,
    /// Scattered to broken cloud, no precipitation.
    PartlyCloudy,
    /// Overcast skies, no precipitation.
    Cloudy,
    /// Any meaningful precipitation, regardless of
    /// cloud cover. Subsumes light rain, heavy rain,
    /// drizzle — the dashboard doesn't distinguish
    /// intensity at v1.
    Rain,
}

impl Condition {
    /// Short human-readable label for the dashboard —
    /// the word that sits next to the big temperature
    /// in the current-conditions panel. Kept short
    /// enough to fit in the right-hand slot at font
    /// size 54 on the 800 × 480 layout without
    /// wrapping.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Sunny => "Sunny",
            Self::PartlyCloudy => "Partly cloudy",
            Self::Cloudy => "Cloudy",
            Self::Rain => "Rain",
        }
    }
}

/// Nine-variant coarse weather taxonomy — the atomic
/// set of "broad buckets" the dashboard knows how to
/// draw an icon for. Produced by [`WmoCode::coarsen`]
/// (when the provider carries a code) or by the
/// fallback cloud+precip heuristic.
///
/// Variant order is "nicer sky first" with
/// [`Self::Unknown`] last — matches the reading order
/// of the coarsen table and keeps the match arms
/// predictable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConditionCategory {
    /// Clear sky.
    Clear,
    /// Scattered to broken cloud.
    PartlyCloudy,
    /// Overcast or near-overcast sky.
    Cloudy,
    /// Visibility-reducing fog or rime fog.
    Fog,
    /// Drizzle (any intensity, including freezing).
    Drizzle,
    /// Rain at or above measurable intensity,
    /// including freezing rain and rain showers.
    Rain,
    /// Snow or snow showers (any intensity).
    Snow,
    /// Thunderstorm, with or without hail.
    Thunderstorm,
    /// Provider-supplied code was unrecognised — i.e.
    /// the wire carried a `weather_code` outside
    /// [`WmoCode`]'s coverage. Fallback numeric signals
    /// don't produce this variant; they produce a
    /// specific bucket.
    Unknown,
}

impl ConditionCategory {
    /// Short human-readable label for the dashboard
    /// condition widget — the word that sits next to
    /// the big temperature. Kept short enough to fit
    /// the right-hand slot at font size 54 on the
    /// 800 × 480 layout without wrapping.
    ///
    /// `Clear` reads as "Sunny" for user-friendliness
    /// (matching the legacy label); every other variant
    /// carries its technical name so the display stays
    /// honest about the provider's classification.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Clear => "Sunny",
            Self::PartlyCloudy => "Partly cloudy",
            Self::Cloudy => "Cloudy",
            Self::Fog => "Fog",
            Self::Drizzle => "Drizzle",
            Self::Rain => "Rain",
            Self::Snow => "Snow",
            Self::Thunderstorm => "Thunderstorm",
            Self::Unknown => "Unknown",
        }
    }
}

/// WMO 4677 weather code — the full lookup table the
/// Open-Meteo `weather_code` field uses.
///
/// Variants are named by their WMO description (kept
/// terse for pattern-match readability). The
/// discriminants match the on-wire numeric codes so
/// [`Self::try_from`] is a single `match` rather than
/// a lookup table.
///
/// Not every row in the WMO 4677 table is present —
/// Open-Meteo documents exactly this 28-entry subset
/// on its forecast docs page. A byte outside the set
/// narrows to [`WeatherCode::Unrecognised`] (carrying
/// the raw byte) at the boundary and surfaces
/// downstream as [`ConditionCategory::Unknown`] via
/// [`classify_category`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WmoCode {
    /// 0 — Clear sky.
    Clear = 0,
    /// 1 — Mainly clear.
    MainlyClear = 1,
    /// 2 — Partly cloudy.
    PartlyCloudy = 2,
    /// 3 — Overcast.
    Overcast = 3,
    /// 45 — Fog.
    Fog = 45,
    /// 48 — Depositing rime fog.
    RimeFog = 48,
    /// 51 — Drizzle: Light.
    DrizzleLight = 51,
    /// 53 — Drizzle: Moderate.
    DrizzleModerate = 53,
    /// 55 — Drizzle: Dense.
    DrizzleDense = 55,
    /// 56 — Freezing Drizzle: Light.
    FreezingDrizzleLight = 56,
    /// 57 — Freezing Drizzle: Dense.
    FreezingDrizzleDense = 57,
    /// 61 — Rain: Slight.
    RainSlight = 61,
    /// 63 — Rain: Moderate.
    RainModerate = 63,
    /// 65 — Rain: Heavy.
    RainHeavy = 65,
    /// 66 — Freezing Rain: Light.
    FreezingRainLight = 66,
    /// 67 — Freezing Rain: Heavy.
    FreezingRainHeavy = 67,
    /// 71 — Snow fall: Slight.
    SnowSlight = 71,
    /// 73 — Snow fall: Moderate.
    SnowModerate = 73,
    /// 75 — Snow fall: Heavy.
    SnowHeavy = 75,
    /// 77 — Snow grains.
    SnowGrains = 77,
    /// 80 — Rain showers: Slight.
    RainShowersSlight = 80,
    /// 81 — Rain showers: Moderate.
    RainShowersModerate = 81,
    /// 82 — Rain showers: Violent.
    RainShowersViolent = 82,
    /// 85 — Snow showers: Slight.
    SnowShowersSlight = 85,
    /// 86 — Snow showers: Heavy.
    SnowShowersHeavy = 86,
    /// 95 — Thunderstorm (slight or moderate).
    Thunderstorm = 95,
    /// 96 — Thunderstorm with slight hail.
    ThunderstormHailSlight = 96,
    /// 99 — Thunderstorm with heavy hail.
    ThunderstormHailHeavy = 99,
}

/// Raw wire-level code that didn't match any variant
/// in [`WmoCode`]. Carries the rejected `u8` so callers
/// can log or surface it — the whole point of the
/// boundary check is that we *know* which value drifted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unrecognised WMO 4677 code: {0}")]
pub struct UnknownWmoCode(pub u8);

impl From<WmoCode> for u8 {
    /// Round-trip back to the on-wire code — safe
    /// because [`WmoCode`] is `#[repr(u8)]` with
    /// explicit discriminants matching the WMO numbers.
    fn from(code: WmoCode) -> Self {
        code as Self
    }
}

impl TryFrom<u8> for WmoCode {
    type Error = UnknownWmoCode;

    /// Narrow a raw wire-level code to a documented
    /// variant. Codes outside the WMO 4677 subset that
    /// Open-Meteo exposes return
    /// `Err(UnknownWmoCode(n))` carrying the rejected
    /// value, so callers can distinguish "provider sent
    /// nothing" from "provider sent a code we don't
    /// recognise".
    fn try_from(code: u8) -> Result<Self, Self::Error> {
        Ok(match code {
            0 => Self::Clear,
            1 => Self::MainlyClear,
            2 => Self::PartlyCloudy,
            3 => Self::Overcast,
            45 => Self::Fog,
            48 => Self::RimeFog,
            51 => Self::DrizzleLight,
            53 => Self::DrizzleModerate,
            55 => Self::DrizzleDense,
            56 => Self::FreezingDrizzleLight,
            57 => Self::FreezingDrizzleDense,
            61 => Self::RainSlight,
            63 => Self::RainModerate,
            65 => Self::RainHeavy,
            66 => Self::FreezingRainLight,
            67 => Self::FreezingRainHeavy,
            71 => Self::SnowSlight,
            73 => Self::SnowModerate,
            75 => Self::SnowHeavy,
            77 => Self::SnowGrains,
            80 => Self::RainShowersSlight,
            81 => Self::RainShowersModerate,
            82 => Self::RainShowersViolent,
            85 => Self::SnowShowersSlight,
            86 => Self::SnowShowersHeavy,
            95 => Self::Thunderstorm,
            96 => Self::ThunderstormHailSlight,
            99 => Self::ThunderstormHailHeavy,
            other => return Err(UnknownWmoCode(other)),
        })
    }
}

impl WmoCode {
    /// Every documented variant in declaration order —
    /// the single source of truth for "which WMO 4677
    /// codes does this project know about". Both
    /// classify tests and icons tests iterate this
    /// constant, so adding a new variant updates both
    /// coverage paths in one shot.
    pub const ALL: &'static [Self] = &[
        Self::Clear,
        Self::MainlyClear,
        Self::PartlyCloudy,
        Self::Overcast,
        Self::Fog,
        Self::RimeFog,
        Self::DrizzleLight,
        Self::DrizzleModerate,
        Self::DrizzleDense,
        Self::FreezingDrizzleLight,
        Self::FreezingDrizzleDense,
        Self::RainSlight,
        Self::RainModerate,
        Self::RainHeavy,
        Self::FreezingRainLight,
        Self::FreezingRainHeavy,
        Self::SnowSlight,
        Self::SnowModerate,
        Self::SnowHeavy,
        Self::SnowGrains,
        Self::RainShowersSlight,
        Self::RainShowersModerate,
        Self::RainShowersViolent,
        Self::SnowShowersSlight,
        Self::SnowShowersHeavy,
        Self::Thunderstorm,
        Self::ThunderstormHailSlight,
        Self::ThunderstormHailHeavy,
    ];

    /// Collapse the detailed code into the nine-way
    /// [`ConditionCategory`] bucket used by the icon
    /// table. Exhaustive: every variant has exactly one
    /// target category, so the compiler catches a
    /// missed arm if a new `WmoCode` is ever added.
    #[must_use]
    pub fn coarsen(self) -> ConditionCategory {
        match self {
            Self::Clear => ConditionCategory::Clear,
            Self::MainlyClear | Self::PartlyCloudy => {
                ConditionCategory::PartlyCloudy
            }
            Self::Overcast => ConditionCategory::Cloudy,
            Self::Fog | Self::RimeFog => ConditionCategory::Fog,
            Self::DrizzleLight
            | Self::DrizzleModerate
            | Self::DrizzleDense
            | Self::FreezingDrizzleLight
            | Self::FreezingDrizzleDense => ConditionCategory::Drizzle,
            Self::RainSlight
            | Self::RainModerate
            | Self::RainHeavy
            | Self::FreezingRainLight
            | Self::FreezingRainHeavy
            | Self::RainShowersSlight
            | Self::RainShowersModerate
            | Self::RainShowersViolent => ConditionCategory::Rain,
            Self::SnowSlight
            | Self::SnowModerate
            | Self::SnowHeavy
            | Self::SnowGrains
            | Self::SnowShowersSlight
            | Self::SnowShowersHeavy => ConditionCategory::Snow,
            Self::Thunderstorm
            | Self::ThunderstormHailSlight
            | Self::ThunderstormHailHeavy => ConditionCategory::Thunderstorm,
        }
    }
}

/// Observed `weather_code` from the provider for a
/// single hour, after boundary narrowing.
///
/// `None` means "provider didn't supply a code". A
/// [`Self::Wmo`] payload means the code is one we
/// recognise. A [`Self::Unrecognised`] payload means
/// the wire carried a byte that's outside
/// [`WmoCode`]'s documented subset — preserved here
/// (rather than collapsed to `None` at the boundary)
/// so the display can surface
/// [`ConditionCategory::Unknown`] instead of silently
/// showing a heuristic-derived category for bad data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherCode {
    /// Recognised WMO 4677 code.
    Wmo(WmoCode),
    /// Byte outside the documented subset; the
    /// [`UnknownWmoCode`] error already logged the
    /// specific value, so we only need to carry the
    /// u8 for diagnostics.
    Unrecognised(u8),
}

impl From<WmoCode> for WeatherCode {
    fn from(code: WmoCode) -> Self {
        Self::Wmo(code)
    }
}

/// Lift a legacy four-variant [`Condition`] into the
/// nine-variant [`ConditionCategory`]. Internal bridge
/// used by [`classify_category`]'s cloud+precip fallback
/// to promote the numeric heuristic's [`Condition`]
/// output into the richer taxonomy. Private to this
/// module — the render layer reads `ConditionCategory`
/// directly now and has no reason to hold a `Condition`.
/// The coarse heuristic can never produce
/// Fog/Drizzle/Snow/Thunderstorm/Unknown — only provider
/// codes can.
#[must_use]
fn condition_to_category(c: Condition) -> ConditionCategory {
    match c {
        Condition::Sunny => ConditionCategory::Clear,
        Condition::PartlyCloudy => ConditionCategory::PartlyCloudy,
        Condition::Cloudy => ConditionCategory::Cloudy,
        Condition::Rain => ConditionCategory::Rain,
    }
}

/// Composite weather classifier. Prefer the provider's
/// WMO code when present; fall back to the numeric
/// cloud+precip heuristic when no code arrived.
///
/// Three cases, each with a distinct display outcome:
///
/// - `Some(WeatherCode::Wmo(code))` → `code.coarsen()`.
/// - `Some(WeatherCode::Unrecognised(_))` →
///   [`ConditionCategory::Unknown`]. Provider sent a
///   code we don't have a glyph for; surface that as a
///   distinct display state rather than lying via the
///   heuristic.
/// - `None` → fallback heuristic over cloud / precip,
///   producing [`ConditionCategory::Clear`],
///   `PartlyCloudy`, `Cloudy`, or `Rain`. Never
///   produces the detailed categories, because numeric
///   signals aren't precise enough — only the provider
///   owns detailed codes.
#[must_use]
pub fn classify_category(
    weather_code: Option<WeatherCode>,
    cloud_pct: f64,
    precip_mmh: f64,
) -> ConditionCategory {
    match weather_code {
        Some(WeatherCode::Wmo(code)) => code.coarsen(),
        Some(WeatherCode::Unrecognised(_)) => ConditionCategory::Unknown,
        None => condition_to_category(classify_weather(cloud_pct, precip_mmh)),
    }
}

/// Eight-point compass direction for wind-from labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compass8 {
    /// North.
    N,
    /// Northeast.
    NE,
    /// East.
    E,
    /// Southeast.
    SE,
    /// South.
    S,
    /// Southwest.
    SW,
    /// West.
    W,
    /// Northwest.
    NW,
}

impl Compass8 {
    /// Short human-readable abbreviation matching the
    /// enum name (`"N"`, `"NE"`, …). Used directly in
    /// the dashboard's wind label.
    #[must_use]
    pub fn abbrev(self) -> &'static str {
        match self {
            Self::N => "N",
            Self::NE => "NE",
            Self::E => "E",
            Self::SE => "SE",
            Self::S => "S",
            Self::SW => "SW",
            Self::W => "W",
            Self::NW => "NW",
        }
    }

    /// Map a compass angle in degrees clockwise from
    /// North (wrapping is tolerated — `-1.0` and
    /// `360.0` both fall back to
    /// [`Compass8::N`]) to the nearest 8-way octant.
    ///
    /// Sectors are 45° wide, each centred on its
    /// compass point — so N covers
    /// `[337.5, 360) ∪ [0, 22.5)`, NE covers
    /// `[22.5, 67.5)`, and so on around the rose.
    /// Half-open on the upper end keeps every angle
    /// assigned to exactly one octant.
    #[must_use]
    pub fn from_degrees(deg: f64) -> Self {
        match deg {
            d if !(0.0..360.0).contains(&d) => Self::N,
            d if d < 22.5 => Self::N,
            d if d < 67.5 => Self::NE,
            d if d < 112.5 => Self::E,
            d if d < 157.5 => Self::SE,
            d if d < 202.5 => Self::S,
            d if d < 247.5 => Self::SW,
            d if d < 292.5 => Self::W,
            d if d < 337.5 => Self::NW,
            _ => Self::N,
        }
    }
}

/// Precipitation threshold (mm/h) above which the
/// condition becomes [`Condition::Rain`] regardless of
/// cloud cover. 0.5 mm/h is the conventional threshold
/// between "trace" and "measurable" precipitation; at
/// e-ink glance distance, anything less isn't worth
/// distinguishing from cloudy.
pub const RAIN_THRESHOLD_MMH: f64 = 0.5;

/// Cloud-cover percentage below which skies count as
/// [`Condition::Sunny`] (when not raining).
pub const SUNNY_CEILING_PCT: f64 = 25.0;

/// Cloud-cover percentage at or above which skies count
/// as fully [`Condition::Cloudy`] (when not raining).
/// Values in `[SUNNY_CEILING_PCT, CLOUDY_FLOOR_PCT)`
/// count as [`Condition::PartlyCloudy`].
pub const CLOUDY_FLOOR_PCT: f64 = 70.0;

/// Classify a single forecast sample into a
/// [`Condition`]. Precipitation dominates: any sample
/// at or above [`RAIN_THRESHOLD_MMH`] is [`Condition::Rain`]
/// no matter the cloud cover. Otherwise cloud
/// percentage picks between [`Condition::Sunny`],
/// [`Condition::PartlyCloudy`], and [`Condition::Cloudy`].
///
/// Thresholds are half-open on the upper end
/// (`cloud < SUNNY_CEILING_PCT` → sunny,
/// `cloud >= CLOUDY_FLOOR_PCT` → cloudy) so the
/// boundary values sit in the higher-severity bucket —
/// rounding never under-reports cloud cover.
#[must_use]
pub fn classify_weather(cloud_pct: f64, precip_mmh: f64) -> Condition {
    if precip_mmh >= RAIN_THRESHOLD_MMH {
        return Condition::Rain;
    }
    if cloud_pct < SUNNY_CEILING_PCT {
        Condition::Sunny
    } else if cloud_pct < CLOUDY_FLOOR_PCT {
        Condition::PartlyCloudy
    } else {
        Condition::Cloudy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── classify_weather ────────────────────────────

    #[test]
    fn clear_sky_is_sunny() {
        assert_eq!(classify_weather(0.0, 0.0), Condition::Sunny);
    }

    #[test]
    fn scattered_cloud_is_sunny_just_below_threshold() {
        assert_eq!(classify_weather(24.9, 0.0), Condition::Sunny);
    }

    #[test]
    fn exactly_25_percent_cloud_tips_into_partly_cloudy() {
        // Boundary: the sunny bucket is `[0, 25)`, so 25
        // itself is partly cloudy. Locks the half-open
        // convention so a refactor can't silently flip it.
        assert_eq!(classify_weather(25.0, 0.0), Condition::PartlyCloudy);
    }

    #[test]
    fn mid_cloud_is_partly_cloudy() {
        assert_eq!(classify_weather(50.0, 0.0), Condition::PartlyCloudy);
    }

    #[test]
    fn cloud_just_below_seventy_is_still_partly_cloudy() {
        assert_eq!(classify_weather(69.9, 0.0), Condition::PartlyCloudy);
    }

    #[test]
    fn exactly_70_percent_cloud_tips_into_cloudy() {
        assert_eq!(classify_weather(70.0, 0.0), Condition::Cloudy);
    }

    #[test]
    fn full_overcast_is_cloudy() {
        assert_eq!(classify_weather(100.0, 0.0), Condition::Cloudy);
    }

    #[test]
    fn light_rain_overrides_clear_sky() {
        // If the model thinks it's raining through a clear
        // sky, we still show rain — the dashboard's job is
        // "what should I wear", not "what's the dominant
        // sky feature".
        assert_eq!(classify_weather(0.0, 0.5), Condition::Rain);
    }

    #[test]
    fn heavy_rain_overrides_full_overcast() {
        assert_eq!(classify_weather(100.0, 5.0), Condition::Rain);
    }

    #[test]
    fn trace_precipitation_below_threshold_does_not_trigger_rain() {
        // 0.4 mm/h is below the rain threshold; with low
        // cloud that's still sunny, not rain.
        assert_eq!(classify_weather(10.0, 0.4), Condition::Sunny);
    }

    // ─── Compass8::from_degrees ──────────────────────

    #[test]
    fn compass_cardinal_points_bucket_correctly() {
        assert_eq!(Compass8::from_degrees(0.0), Compass8::N);
        assert_eq!(Compass8::from_degrees(45.0), Compass8::NE);
        assert_eq!(Compass8::from_degrees(90.0), Compass8::E);
        assert_eq!(Compass8::from_degrees(135.0), Compass8::SE);
        assert_eq!(Compass8::from_degrees(180.0), Compass8::S);
        assert_eq!(Compass8::from_degrees(225.0), Compass8::SW);
        assert_eq!(Compass8::from_degrees(270.0), Compass8::W);
        assert_eq!(Compass8::from_degrees(315.0), Compass8::NW);
    }

    #[test]
    fn compass_sector_boundaries_round_into_higher_numbered_octant() {
        // Half-open convention: boundary angles sit in
        // the next octant. Locking this so refactors
        // don't silently shift the rose.
        assert_eq!(Compass8::from_degrees(22.5), Compass8::NE);
        assert_eq!(Compass8::from_degrees(67.5), Compass8::E);
        assert_eq!(Compass8::from_degrees(337.5), Compass8::N);
    }

    #[test]
    fn compass_north_wraps_around_3_3_7_5_boundary() {
        // 337.4 → NW, 337.5 → N.
        assert_eq!(Compass8::from_degrees(337.4), Compass8::NW);
        assert_eq!(Compass8::from_degrees(337.5), Compass8::N);
        assert_eq!(Compass8::from_degrees(359.99), Compass8::N);
    }

    #[test]
    fn compass_out_of_range_angles_default_to_north() {
        // Defence in depth — the snapshot adapter
        // normalises via `rem_euclid`, so in practice
        // `from_degrees` never sees an out-of-range
        // input. Keeping the match total means a future
        // provider that forgets to wrap still gets a
        // sane fallback.
        assert_eq!(Compass8::from_degrees(-1.0), Compass8::N);
        assert_eq!(Compass8::from_degrees(360.0), Compass8::N);
        assert_eq!(Compass8::from_degrees(720.0), Compass8::N);
    }

    // ─── WmoCode::try_from + From<WmoCode> for u8 ────

    #[test]
    fn wmo_all_list_matches_try_from_round_trip() {
        // Round-trip every variant in `WmoCode::ALL`:
        // cast to u8, parse back via `TryFrom`, confirm
        // equality. Catches typos in either half of the
        // match as well as a drift between `ALL` and the
        // `TryFrom` table.
        for &code in WmoCode::ALL {
            let n: u8 = u8::from(code);
            let round = WmoCode::try_from(n)
                .expect("WmoCode::ALL codes must parse back");
            assert_eq!(round, code, "round-trip mismatch for {code:?}");
        }
    }

    #[test]
    fn wmo_try_from_rejects_codes_outside_the_table_with_payload() {
        // Gaps in the WMO subset (e.g. 4, 44, 50, 52,
        // 100, 255) must narrow to Err carrying the
        // rejected byte.
        for n in [
            4_u8, 44, 50, 52, 58, 68, 72, 78, 87, 94, 97, 98, 100, 200, 255,
        ] {
            let err = WmoCode::try_from(n).unwrap_err();
            assert_eq!(
                err,
                UnknownWmoCode(n),
                "expected rejected byte in error payload",
            );
            // Display carries the actionable value — a
            // logger can grep for the number rather than
            // having to attach it separately.
            assert!(
                err.to_string().contains(&n.to_string()),
                "error message should mention code {n}: {err}",
            );
        }
    }

    // ─── WmoCode::coarsen ───────────────────────────

    #[test]
    fn coarsen_follows_handoff_mapping_exhaustively() {
        // Lock the mapping from HANDOFF.md PR 2 table.
        // Driver pattern: each (code, expected) pair
        // is a single row of the coarsen table. The
        // `len() == WmoCode::ALL.len()` assertion below
        // forces this table to grow in lockstep with a
        // new variant.
        let pairs: &[(u8, ConditionCategory)] = &[
            (0, ConditionCategory::Clear),
            (1, ConditionCategory::PartlyCloudy),
            (2, ConditionCategory::PartlyCloudy),
            (3, ConditionCategory::Cloudy),
            (45, ConditionCategory::Fog),
            (48, ConditionCategory::Fog),
            (51, ConditionCategory::Drizzle),
            (53, ConditionCategory::Drizzle),
            (55, ConditionCategory::Drizzle),
            (56, ConditionCategory::Drizzle),
            (57, ConditionCategory::Drizzle),
            (61, ConditionCategory::Rain),
            (63, ConditionCategory::Rain),
            (65, ConditionCategory::Rain),
            (66, ConditionCategory::Rain),
            (67, ConditionCategory::Rain),
            (71, ConditionCategory::Snow),
            (73, ConditionCategory::Snow),
            (75, ConditionCategory::Snow),
            (77, ConditionCategory::Snow),
            (80, ConditionCategory::Rain),
            (81, ConditionCategory::Rain),
            (82, ConditionCategory::Rain),
            (85, ConditionCategory::Snow),
            (86, ConditionCategory::Snow),
            (95, ConditionCategory::Thunderstorm),
            (96, ConditionCategory::Thunderstorm),
            (99, ConditionCategory::Thunderstorm),
        ];
        assert_eq!(
            pairs.len(),
            WmoCode::ALL.len(),
            "coarsen table must list every documented code",
        );
        for &(n, expected) in pairs {
            let code = WmoCode::try_from(n).unwrap();
            assert_eq!(
                code.coarsen(),
                expected,
                "coarsen mismatch for code {n}",
            );
        }
    }

    // ─── classify_category ──────────────────────────

    #[test]
    fn category_prefers_the_provider_code_over_numeric_signals() {
        // Provider says thunderstorm but cloud+precip
        // read as sunny — the code wins.
        let got = classify_category(
            Some(WeatherCode::Wmo(WmoCode::Thunderstorm)),
            0.0,
            0.0,
        );
        assert_eq!(got, ConditionCategory::Thunderstorm);
    }

    #[test]
    fn category_from_wmocode_via_into_shorthand() {
        // `From<WmoCode> for WeatherCode` is the
        // ergonomic path for callers who already hold a
        // parsed WmoCode — less punctuation at call
        // sites than the explicit `Wmo(...)` wrap.
        let got: WeatherCode = WmoCode::Fog.into();
        assert_eq!(
            classify_category(Some(got), 0.0, 0.0),
            ConditionCategory::Fog,
        );
    }

    #[test]
    fn category_surfaces_unknown_for_unrecognised_provider_code() {
        // Provider sent a byte that narrowing rejected —
        // don't silently substitute a heuristic-derived
        // category. Surface Unknown so bad provider data
        // is visible at a glance rather than merged into
        // "partly cloudy".
        let got = classify_category(
            Some(WeatherCode::Unrecognised(4)),
            50.0, // would read as PartlyCloudy under fallback
            0.0,
        );
        assert_eq!(got, ConditionCategory::Unknown);
    }

    #[test]
    fn category_falls_back_when_provider_code_is_absent() {
        // No code → rely on the existing heuristic,
        // promoted to ConditionCategory.
        assert_eq!(classify_category(None, 0.0, 5.0), ConditionCategory::Rain,);
        assert_eq!(
            classify_category(None, 10.0, 0.0),
            ConditionCategory::Clear,
        );
        assert_eq!(
            classify_category(None, 100.0, 0.0),
            ConditionCategory::Cloudy,
        );
    }

    #[test]
    fn fallback_cannot_produce_detailed_categories() {
        // The numeric heuristic is too coarse to emit
        // Fog, Drizzle, Snow, Thunderstorm, or Unknown
        // — only the provider code can. Locks the
        // "provider owns detail" invariant.
        let cloud_grid = [0.0, 24.9, 25.0, 50.0, 69.9, 70.0, 100.0];
        let precip_grid = [0.0, 0.4, 0.5, 5.0];
        for c in cloud_grid {
            for p in precip_grid {
                let cat = classify_category(None, c, p);
                assert!(
                    matches!(
                        cat,
                        ConditionCategory::Clear
                            | ConditionCategory::PartlyCloudy
                            | ConditionCategory::Cloudy
                            | ConditionCategory::Rain,
                    ),
                    "fallback produced {cat:?} for cloud={c}, precip={p}",
                );
            }
        }
    }

    // ─── condition_to_category ──────────────────────

    #[test]
    fn condition_promotes_to_the_expected_category() {
        // Locks the coarse fallback path inside
        // `classify_category(None, …)`; the numeric
        // heuristic classifies into `Condition`, this
        // mapper lifts into `ConditionCategory`.
        assert_eq!(
            condition_to_category(Condition::Sunny),
            ConditionCategory::Clear,
        );
        assert_eq!(
            condition_to_category(Condition::PartlyCloudy),
            ConditionCategory::PartlyCloudy,
        );
        assert_eq!(
            condition_to_category(Condition::Cloudy),
            ConditionCategory::Cloudy,
        );
        assert_eq!(
            condition_to_category(Condition::Rain),
            ConditionCategory::Rain,
        );
    }

    #[test]
    fn compass_abbrev_matches_variant_name() {
        // Label-in-enum invariant: `abbrev()` must return
        // exactly the Debug name, because the SVG builder
        // embeds it verbatim. A mismatch here would show
        // a confusing label like "NE" for `Compass8::E`.
        for (variant, expected) in [
            (Compass8::N, "N"),
            (Compass8::NE, "NE"),
            (Compass8::E, "E"),
            (Compass8::SE, "SE"),
            (Compass8::S, "S"),
            (Compass8::SW, "SW"),
            (Compass8::W, "W"),
            (Compass8::NW, "NW"),
        ] {
            assert_eq!(variant.abbrev(), expected);
        }
    }
}
