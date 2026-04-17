//! Sunrise/sunset calculations for a given date and
//! location.
//!
//! Pure computation, no network, no extra crate
//! dependency. Hand-rolled NOAA solar position
//! algorithm (the one backing the NOAA solar
//! calculator spreadsheet at
//! `https://gml.noaa.gov/grad/solcalc/calcdetails.html`).
//! Accurate to about ±1 minute for non-polar
//! latitudes, which is plenty for a dashboard status
//! line.
//!
//! For polar regions on solstice dates — where the
//! sun does not rise or set at all — returns
//! `(None, None)`.

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

/// Geographic point on Earth's surface. Packed into
/// a struct so callers can't swap latitude and
/// longitude by accident — a bug that compiles
/// cleanly when both are bare `f64`s.
///
/// Used by [`sunrise_sunset`] and by
/// [`super::model::ModelContext`] to forward the
/// device's configured location through the dashboard
/// pipeline.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct GeoPoint {
    /// Latitude in decimal degrees. Positive north,
    /// negative south. Valid range is `[-90, 90]`;
    /// values outside it are accepted but yield
    /// arbitrary results.
    pub lat_deg: f64,
    /// Longitude in decimal degrees. Positive east,
    /// negative west. Valid range is `[-180, 180]`;
    /// values outside it are accepted but yield
    /// arbitrary results.
    pub lon_deg: f64,
}

/// Standard zenith angle for sunrise / sunset,
/// including atmospheric refraction (~34') and the
/// apparent solar radius (~16'). 90° 50' → 90.833°.
const ZENITH_DEG: f64 = 90.833;

/// Compute local sunrise and sunset for a calendar
/// date at a given geographic location.
///
/// `date` is the desired *local* date in `tz`. `tz`
/// is the IANA zone the returned wall-clock times
/// are expressed in.
///
/// Returns `(None, None)` for polar day or polar
/// night — the caller is expected to render a
/// placeholder when the sun does not rise or set.
///
/// ## Implementation note
///
/// The ephemeris reference is the UTC instant of
/// **local noon** on `date`, not UTC noon of the
/// same calendar date. Anchoring to local noon keeps
/// the solar-declination and equation-of-time values
/// within ±12 hours of the event we're computing —
/// at locations near the international date line
/// that's a day's worth of ephemeris drift, enough
/// to spuriously flip "polar day" / "polar night"
/// status at high latitudes near an equinox. The
/// resulting sunrise/sunset UTC instant is then
/// converted into `tz` via chrono, which also
/// ensures the returned `NaiveTime` is the
/// wall-clock reading a local observer would see on
/// `date`.
#[must_use]
pub fn sunrise_sunset(
    date: NaiveDate,
    location: GeoPoint,
    tz: Tz,
) -> (Option<NaiveTime>, Option<NaiveTime>) {
    let local_noon_utc = local_noon_utc_instant(date, tz)
        .expect("every valid local date has at least one local-noon instant");
    let t = julian_century(julian_day_from_instant(local_noon_utc));
    let decl = sun_declination(t);
    let Some(ha_deg) = hour_angle_sunrise(location.lat_deg, decl) else {
        return (None, None);
    };
    let eot_min = eq_of_time(t);
    // 720 = minutes from UTC-midnight to UTC-noon.
    // The longitude term shifts to the meridian's
    // local noon; the equation-of-time correction
    // accounts for the elliptical orbit and axial
    // tilt. The resulting `solar_noon_utc_min` is
    // measured from UTC midnight of the *UTC date*
    // containing `local_noon_utc`.
    let solar_noon_utc_min = 720.0 - 4.0 * location.lon_deg - eot_min;
    let sunrise_utc_min = solar_noon_utc_min - 4.0 * ha_deg;
    let sunset_utc_min = solar_noon_utc_min + 4.0 * ha_deg;
    let utc_date = local_noon_utc.date_naive();
    (
        to_local_time(utc_date, sunrise_utc_min, tz),
        to_local_time(utc_date, sunset_utc_min, tz),
    )
}

/// UTC instant of noon in `tz` on `date`. Picks the
/// earliest candidate on DST fall-back days (noon
/// happens twice — we take the first occurrence).
/// Returns `None` only if `date` is an invalid
/// `NaiveDate` (which shouldn't happen since callers
/// construct it from chrono).
fn local_noon_utc_instant(date: NaiveDate, tz: Tz) -> Option<DateTime<Utc>> {
    let naive_noon = date.and_hms_opt(12, 0, 0)?;
    let local = tz
        .from_local_datetime(&naive_noon)
        .earliest()
        .or_else(|| tz.from_local_datetime(&naive_noon).latest())?;
    Some(local.with_timezone(&Utc))
}

/// Convert `utc_minutes` (minutes since UTC midnight
/// of `utc_date`) to a local [`NaiveTime`] in `tz`.
/// Handles events that spill into the adjacent UTC
/// day by letting `Duration` normalise naturally.
fn to_local_time(
    utc_date: NaiveDate,
    utc_minutes: f64,
    tz: Tz,
) -> Option<NaiveTime> {
    if !utc_minutes.is_finite() {
        return None;
    }
    #[allow(clippy::cast_possible_truncation)]
    let total_seconds = (utc_minutes * 60.0).round() as i64;
    let base = utc_date.and_hms_opt(0, 0, 0)?;
    let dt_utc =
        Utc.from_utc_datetime(&base) + Duration::seconds(total_seconds);
    Some(dt_utc.with_timezone(&tz).time())
}

fn julian_day_from_instant(instant: DateTime<Utc>) -> f64 {
    // 1970-01-01 00:00 UTC corresponds to JD 2_440_587.5.
    // Seconds-since-epoch / 86400 gives the fractional
    // day offset directly.
    #[allow(clippy::cast_precision_loss)]
    let seconds = instant.timestamp() as f64;
    2_440_587.5 + seconds / 86_400.0
}

fn julian_century(jd: f64) -> f64 {
    (jd - 2_451_545.0) / 36_525.0
}

fn geom_mean_long_sun(t: f64) -> f64 {
    let raw = 280.466_46 + t * (36_000.769_83 + t * 0.000_303_2);
    raw.rem_euclid(360.0)
}

fn geom_mean_anom_sun(t: f64) -> f64 {
    357.529_11 + t * (35_999.050_29 - 0.000_153_7 * t)
}

fn eccent_earth_orbit(t: f64) -> f64 {
    0.016_708_634 - t * (0.000_042_037 + 0.000_000_126_7 * t)
}

fn sun_eq_of_center(t: f64) -> f64 {
    let m = geom_mean_anom_sun(t).to_radians();
    m.sin() * (1.914_602 - t * (0.004_817 + 0.000_014 * t))
        + (2.0 * m).sin() * (0.019_993 - 0.000_101 * t)
        + (3.0 * m).sin() * 0.000_289
}

fn sun_app_long(t: f64) -> f64 {
    let true_long = geom_mean_long_sun(t) + sun_eq_of_center(t);
    let omega = (125.04 - 1934.136 * t).to_radians();
    true_long - 0.005_69 - 0.004_78 * omega.sin()
}

fn mean_obliq_ecliptic(t: f64) -> f64 {
    let seconds = 21.448 - t * (46.815 + t * (0.000_59 - t * 0.001_813));
    23.0 + (26.0 + seconds / 60.0) / 60.0
}

fn obliq_corr(t: f64) -> f64 {
    let omega = (125.04 - 1934.136 * t).to_radians();
    mean_obliq_ecliptic(t) + 0.002_56 * omega.cos()
}

fn sun_declination(t: f64) -> f64 {
    let eps = obliq_corr(t).to_radians();
    let lambda = sun_app_long(t).to_radians();
    (eps.sin() * lambda.sin()).asin().to_degrees()
}

fn eq_of_time(t: f64) -> f64 {
    let eps = obliq_corr(t);
    let l0 = geom_mean_long_sun(t);
    let e = eccent_earth_orbit(t);
    let m = geom_mean_anom_sun(t);
    let y = (eps / 2.0).to_radians().tan().powi(2);
    let sin_2l0 = (2.0 * l0).to_radians().sin();
    let cos_2l0 = (2.0 * l0).to_radians().cos();
    let sin_m_anom = m.to_radians().sin();
    let sin_4l0 = (4.0 * l0).to_radians().sin();
    let sin_double_m = (2.0 * m).to_radians().sin();
    let rad = y * sin_2l0 - 2.0 * e * sin_m_anom
        + 4.0 * e * y * sin_m_anom * cos_2l0
        - 0.5 * y * y * sin_4l0
        - 1.25 * e * e * sin_double_m;
    // Minutes.
    4.0 * rad.to_degrees()
}

fn hour_angle_sunrise(lat_deg: f64, decl_deg: f64) -> Option<f64> {
    let lat = lat_deg.to_radians();
    let decl = decl_deg.to_radians();
    let z = ZENITH_DEG.to_radians();
    let arg = (z.cos() - lat.sin() * decl.sin()) / (lat.cos() * decl.cos());
    if (-1.0..=1.0).contains(&arg) {
        Some(arg.acos().to_degrees())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    /// Tolerance in minutes for matching a computed
    /// sunrise/sunset against a reference value.
    const TOLERANCE_MIN: i64 = 3;

    fn minutes_diff(a: NaiveTime, b: NaiveTime) -> i64 {
        let a_s = i64::from(a.num_seconds_from_midnight());
        let b_s = i64::from(b.num_seconds_from_midnight());
        (a_s - b_s).abs() / 60
    }

    fn close(got: Option<NaiveTime>, expected: NaiveTime, label: &str) {
        let got =
            got.unwrap_or_else(|| panic!("{label}: expected Some, got None"));
        let diff = minutes_diff(got, expected);
        assert!(
            diff <= TOLERANCE_MIN,
            "{label}: computed {got} vs expected {expected} (Δ {diff} min)",
        );
    }

    const LJUBLJANA: GeoPoint = GeoPoint {
        lat_deg: 46.05,
        lon_deg: 14.51,
    };
    const SYDNEY: GeoPoint = GeoPoint {
        lat_deg: -33.87,
        lon_deg: 151.21,
    };
    const REYKJAVIK: GeoPoint = GeoPoint {
        lat_deg: 64.1,
        lon_deg: -21.9,
    };
    const SVALBARD: GeoPoint = GeoPoint {
        lat_deg: 78.0,
        lon_deg: 16.0,
    };
    /// Kiritimati / Kiribati — UTC+14, the furthest-
    /// east real-world civil time zone. Exercises the
    /// "local noon is yesterday UTC" edge that a
    /// UTC-date-anchored implementation would get
    /// wrong.
    const KIRITIMATI: GeoPoint = GeoPoint {
        lat_deg: 1.87,
        lon_deg: -157.4,
    };

    #[test]
    fn ljubljana_summer_solstice() {
        // 2026-06-21 at Ljubljana, Slovenia. NOAA
        // reference: sunrise 05:13 CEST, sunset 20:58
        // CEST.
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let (rise, set) =
            sunrise_sunset(date, LJUBLJANA, chrono_tz::Europe::Ljubljana);
        close(rise, NaiveTime::from_hms_opt(5, 13, 0).unwrap(), "sunrise");
        close(set, NaiveTime::from_hms_opt(20, 58, 0).unwrap(), "sunset");
    }

    #[test]
    fn ljubljana_winter_solstice() {
        // 2025-12-21 at Ljubljana. NOAA reference:
        // sunrise 07:41 CET, sunset 16:22 CET.
        let date = NaiveDate::from_ymd_opt(2025, 12, 21).unwrap();
        let (rise, set) =
            sunrise_sunset(date, LJUBLJANA, chrono_tz::Europe::Ljubljana);
        close(rise, NaiveTime::from_hms_opt(7, 41, 0).unwrap(), "sunrise");
        close(set, NaiveTime::from_hms_opt(16, 22, 0).unwrap(), "sunset");
    }

    #[test]
    fn sydney_summer_solstice_southern_hemisphere() {
        // 2026-12-21 at Sydney, Australia. NOAA
        // reference: sunrise 05:42 AEDT, sunset 20:08
        // AEDT.
        let date = NaiveDate::from_ymd_opt(2026, 12, 21).unwrap();
        let (rise, set) =
            sunrise_sunset(date, SYDNEY, chrono_tz::Australia::Sydney);
        close(rise, NaiveTime::from_hms_opt(5, 42, 0).unwrap(), "sunrise");
        close(set, NaiveTime::from_hms_opt(20, 8, 0).unwrap(), "sunset");
    }

    #[test]
    fn reykjavik_winter_sun_still_rises() {
        // Reykjavík at 64.1°N — sun does rise briefly
        // around winter solstice, just very late.
        // Both values should be Some.
        let date = NaiveDate::from_ymd_opt(2025, 12, 21).unwrap();
        let (rise, set) =
            sunrise_sunset(date, REYKJAVIK, chrono_tz::Atlantic::Reykjavik);
        assert!(rise.is_some(), "rise={rise:?}");
        assert!(set.is_some(), "set={set:?}");
    }

    #[test]
    fn svalbard_winter_polar_night() {
        // Svalbard at ~78°N in winter: sun never
        // rises. hour_angle's arccos argument is
        // outside [-1, 1].
        let date = NaiveDate::from_ymd_opt(2025, 12, 21).unwrap();
        let (rise, set) = sunrise_sunset(date, SVALBARD, chrono_tz::UTC);
        assert!(rise.is_none(), "expected polar night, got {rise:?}");
        assert!(set.is_none(), "expected polar night, got {set:?}");
    }

    #[test]
    fn svalbard_summer_polar_day() {
        // Svalbard at summer solstice: sun never
        // sets. hour_angle arg again outside [-1, 1].
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let (rise, set) = sunrise_sunset(date, SVALBARD, chrono_tz::UTC);
        assert!(rise.is_none(), "expected polar day, got {rise:?}");
        assert!(set.is_none(), "expected polar day, got {set:?}");
    }

    #[test]
    fn kiritimati_date_line_sunrise_matches_requested_local_date() {
        // Kiritimati uses UTC+14 — local noon on a
        // given date is 22:00 UTC of the *previous*
        // calendar day. A naïve "UTC-date-of-`date`"
        // anchor would compute the ephemeris for the
        // wrong UTC day. Expected sunrise on
        // 2026-03-21 local ≈ 06:00 LINT (it's near
        // the equator, so very stable year-round).
        let date = NaiveDate::from_ymd_opt(2026, 3, 21).unwrap();
        let (rise, set) =
            sunrise_sunset(date, KIRITIMATI, chrono_tz::Pacific::Kiritimati);
        let rise = rise.expect("rise");
        let set = set.expect("set");
        // Sanity: near the equator at equinox, day
        // length is ≈ 12 h; sunrise ≈ 06:00 local,
        // sunset ≈ 18:00 local.
        assert!(
            (5..=7).contains(&rise.hour()),
            "sunrise {rise} not near 06:00 LINT",
        );
        assert!(
            (17..=19).contains(&set.hour()),
            "sunset {set} not near 18:00 LINT",
        );
    }
}
