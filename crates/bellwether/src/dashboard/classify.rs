//! Weather classification + compass-direction bucketing.
//!
//! Pure functions that translate already-normalised
//! forecast values (°C, km/h, mm, compass degrees) into
//! the display-domain enums the dashboard consumes.
//! Unit conversion and u/v → (magnitude, direction)
//! arithmetic live in the provider adapter (see
//! `crate::clients::windy::snapshot`); this module is
//! pure display-layer logic.

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
