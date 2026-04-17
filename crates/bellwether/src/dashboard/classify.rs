//! Weather classification + wind-vector conversion.
//!
//! Pure functions that translate raw Windy numeric
//! outputs into the display-domain enums the dashboard
//! consumes. No I/O, no allocation, no dependencies on
//! the rest of the crate — trivially table-testable.

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

/// Below this wind-vector magnitude (m/s) the wind is
/// treated as calm; direction becomes meaningless and
/// the function returns the default [`Compass8::N`].
/// 0.1 m/s is well below any forecast model's
/// resolution.
const CALM_THRESHOLD_MS: f64 = 0.1;

/// Convert Windy's wind vector components into a
/// human-facing `(speed_kmh, from_direction)` pair.
///
/// ## Windy's convention
///
/// `wind_u-surface` is the **zonal** component
/// (positive **eastward**, m/s). `wind_v-surface` is
/// the **meridional** component (positive **northward**,
/// m/s). So `(u, v) = (10, 0)` means the air is moving
/// eastward at 10 m/s; meteorologically that's a **west
/// wind** (a wind is named by the direction it comes
/// *from*).
///
/// ## Return value
///
/// - `speed_kmh`: `sqrt(u² + v²) * 3.6`.
/// - `from_direction`: the compass octant the wind is
///   blowing **from**, in the standard 8-way rose with
///   45° sectors centred on each cardinal and ordinal
///   direction.
///
/// Vectors with magnitude below [`CALM_THRESHOLD_MS`]
/// return `(0.0, Compass8::N)` — at sub-noise speeds
/// the direction is an artefact of floating-point dust.
#[must_use]
pub fn wind_to_compass(u: f64, v: f64) -> (f64, Compass8) {
    let speed_ms = u.hypot(v);
    if speed_ms < CALM_THRESHOLD_MS {
        return (0.0, Compass8::N);
    }
    let speed_kmh = speed_ms * 3.6;
    // atan2(u, v) gives the direction the wind is blowing
    // *toward*, measured clockwise from north. Adding 180°
    // flips it to the direction the wind is coming *from*,
    // which is the label people expect.
    let to_deg = u.atan2(v).to_degrees();
    let from_deg = (to_deg + 180.0).rem_euclid(360.0);
    (speed_kmh, bucket_compass8(from_deg))
}

/// Map a compass angle in degrees clockwise from North
/// (`[0, 360)`) to the nearest 8-way octant. Sectors are
/// 45° wide, each centred on its compass point — so N
/// covers `[337.5, 360) ∪ [0, 22.5)`, NE covers
/// `[22.5, 67.5)`, and so on around the rose. Half-open
/// on the upper end keeps every angle assigned to
/// exactly one octant.
fn bucket_compass8(deg: f64) -> Compass8 {
    match deg {
        d if !(0.0..360.0).contains(&d) => Compass8::N,
        d if d < 22.5 => Compass8::N,
        d if d < 67.5 => Compass8::NE,
        d if d < 112.5 => Compass8::E,
        d if d < 157.5 => Compass8::SE,
        d if d < 202.5 => Compass8::S,
        d if d < 247.5 => Compass8::SW,
        d if d < 292.5 => Compass8::W,
        d if d < 337.5 => Compass8::NW,
        _ => Compass8::N,
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

    // ─── wind_to_compass ─────────────────────────────

    #[test]
    fn calm_wind_returns_zero_speed_north() {
        assert_eq!(wind_to_compass(0.0, 0.0), (0.0, Compass8::N));
    }

    #[test]
    fn sub_noise_wind_is_treated_as_calm() {
        // Less than 0.1 m/s magnitude: direction is
        // floating-point dust, not a real reading.
        let (kmh, dir) = wind_to_compass(0.05, 0.05);
        assert_eq!((kmh, dir), (0.0, Compass8::N));
    }

    #[test]
    fn northward_flow_is_a_south_wind() {
        // Air moving toward north (v=+10) means wind is
        // blowing *from* the south; label: S.
        let (kmh, dir) = wind_to_compass(0.0, 10.0);
        assert!((kmh - 36.0).abs() < 1e-9, "kmh={kmh}");
        assert_eq!(dir, Compass8::S);
    }

    #[test]
    fn southward_flow_is_a_north_wind() {
        let (kmh, dir) = wind_to_compass(0.0, -10.0);
        assert!((kmh - 36.0).abs() < 1e-9);
        assert_eq!(dir, Compass8::N);
    }

    #[test]
    fn eastward_flow_is_a_west_wind() {
        let (_, dir) = wind_to_compass(10.0, 0.0);
        assert_eq!(dir, Compass8::W);
    }

    #[test]
    fn westward_flow_is_an_east_wind() {
        let (_, dir) = wind_to_compass(-10.0, 0.0);
        assert_eq!(dir, Compass8::E);
    }

    #[test]
    fn northeastward_flow_is_a_southwest_wind() {
        // 45° diagonal: (10, 10) moves NE → from SW.
        let (kmh, dir) = wind_to_compass(10.0, 10.0);
        // sqrt(200) * 3.6 = 50.91...
        assert!((kmh - 50.911_688).abs() < 1e-5, "kmh={kmh}");
        assert_eq!(dir, Compass8::SW);
    }

    #[test]
    fn northwestward_flow_is_a_southeast_wind() {
        let (_, dir) = wind_to_compass(-10.0, 10.0);
        assert_eq!(dir, Compass8::SE);
    }

    #[test]
    fn southeastward_flow_is_a_northwest_wind() {
        let (_, dir) = wind_to_compass(10.0, -10.0);
        assert_eq!(dir, Compass8::NW);
    }

    #[test]
    fn southwestward_flow_is_a_northeast_wind() {
        let (_, dir) = wind_to_compass(-10.0, -10.0);
        assert_eq!(dir, Compass8::NE);
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

    #[test]
    fn sector_boundaries_round_into_higher_numbered_octant() {
        // 22.5° is the N/NE boundary; the half-open
        // convention assigns it to NE. Locking this
        // ensures refactors don't silently shift the
        // rose.
        assert_eq!(bucket_compass8(22.5), Compass8::NE);
        assert_eq!(bucket_compass8(67.5), Compass8::E);
        assert_eq!(bucket_compass8(337.5), Compass8::N);
    }

    #[test]
    fn out_of_range_angles_default_to_north() {
        // Defence in depth — `rem_euclid` normalises
        // angles, so in practice `bucket_compass8` never
        // sees an out-of-range input, but guarding the
        // match keeps the function total.
        assert_eq!(bucket_compass8(-1.0), Compass8::N);
        assert_eq!(bucket_compass8(360.0), Compass8::N);
    }
}
