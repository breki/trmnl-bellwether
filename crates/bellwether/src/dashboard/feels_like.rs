//! Apparent-temperature ("feels like") calculation.
//!
//! Pure function combining temperature, humidity, and
//! wind into a single value that better reflects
//! perceived comfort than the raw thermometer
//! reading. Uses the two NWS formulas with their
//! standard applicability thresholds:
//!
//! - **Heat index** (Rothfusz regression) when
//!   `temp ≥ 26.67 °C` *and* `humidity ≥ 40 %`.
//! - **Wind chill** (NWS 2001 formula) when
//!   `temp ≤ 10 °C` *and* `wind > 4.8 km/h`.
//! - Otherwise the raw temperature is returned
//!   unchanged.
//!
//! Both formulas are published for Fahrenheit /
//! miles-per-hour inputs; this module handles the
//! unit conversions internally so callers stay in
//! the project's native metric units.
//!
//! Any non-finite output (NaN / ±∞ from
//! out-of-domain inputs) is discarded and the raw
//! temperature is returned instead. Callers can rely
//! on the output being finite whenever `temp_c` is.

/// Temperature at or above which the heat-index
/// branch can apply (26.67 °C = 80 °F).
const HEAT_INDEX_TEMP_THRESHOLD_C: f64 = 26.67;

/// Humidity at or above which the heat-index branch
/// can apply. Below this, Rothfusz is not calibrated.
const HEAT_INDEX_RH_THRESHOLD_PCT: f64 = 40.0;

/// Temperature at or below which the wind-chill
/// branch can apply (10 °C = 50 °F).
const WIND_CHILL_TEMP_THRESHOLD_C: f64 = 10.0;

/// Wind speed above which the wind-chill branch can
/// apply (4.8 km/h ≈ 3 mph). Below this, wind chill
/// is not meaningfully different from raw temp.
const WIND_CHILL_WIND_THRESHOLD_KMH: f64 = 4.8;

/// km/h per mile-per-hour. Used to convert wind
/// speed into the NWS wind-chill formula's native
/// units.
const KMH_PER_MPH: f64 = 1.609_344;

/// Compute the apparent temperature in °C.
///
/// `temp_c` is the raw air temperature in °C.
/// `humidity_pct` is relative humidity (0–100); pass
/// `None` when unavailable — the heat-index branch
/// is then skipped. `wind_kmh` is the wind speed
/// in km/h; `0.0` is treated as "calm" and disables
/// the wind-chill branch.
///
/// Returns the raw `temp_c` when neither branch
/// applies, when the formula produces a non-finite
/// result, or when `temp_c` itself is non-finite.
#[must_use]
pub fn apparent_temperature_c(
    temp_c: f64,
    humidity_pct: Option<f64>,
    wind_kmh: f64,
) -> f64 {
    if !temp_c.is_finite() {
        return temp_c;
    }
    if temp_c >= HEAT_INDEX_TEMP_THRESHOLD_C
        && let Some(rh) = humidity_pct
        && rh.is_finite()
        && rh >= HEAT_INDEX_RH_THRESHOLD_PCT
    {
        let hi = heat_index_c(temp_c, rh);
        if hi.is_finite() {
            return hi;
        }
    }
    if temp_c <= WIND_CHILL_TEMP_THRESHOLD_C
        && wind_kmh.is_finite()
        && wind_kmh > WIND_CHILL_WIND_THRESHOLD_KMH
    {
        let wc = wind_chill_c(temp_c, wind_kmh);
        if wc.is_finite() {
            return wc;
        }
    }
    temp_c
}

/// Rothfusz heat-index regression (NWS). Inputs
/// converted from °C / percent to °F internally; the
/// result is converted back to °C.
fn heat_index_c(temp_c: f64, rh_pct: f64) -> f64 {
    let t = temp_c * 9.0 / 5.0 + 32.0;
    let r = rh_pct;
    let hi_f = -42.379 + 2.049_015_23 * t + 10.143_331_27 * r
        - 0.224_755_41 * t * r
        - 0.006_837_83 * t * t
        - 0.054_817_17 * r * r
        + 0.001_228_74 * t * t * r
        + 0.000_852_82 * t * r * r
        - 0.000_001_99 * t * t * r * r;
    (hi_f - 32.0) * 5.0 / 9.0
}

/// NWS 2001 wind-chill formula. Inputs converted
/// from °C / km/h to °F / mph internally; the result
/// is converted back to °C.
fn wind_chill_c(temp_c: f64, wind_kmh: f64) -> f64 {
    let t = temp_c * 9.0 / 5.0 + 32.0;
    let v = wind_kmh / KMH_PER_MPH;
    let v_pow = v.powf(0.16);
    let wc_f = 35.74 + 0.6215 * t - 35.75 * v_pow + 0.427_5 * t * v_pow;
    (wc_f - 32.0) * 5.0 / 9.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compare two temperatures within 0.5 °C — the
    /// round-off from the °F↔°C conversions in the
    /// formulas is the dominant error source; anything
    /// tighter would be flaky.
    fn close(got: f64, expected: f64, label: &str) {
        assert!(
            (got - expected).abs() < 0.5,
            "{label}: got {got:.2}, expected ~{expected:.2}",
        );
    }

    #[test]
    fn mild_weather_is_just_raw_temp() {
        // 20 °C, 50 % RH, 10 km/h wind — comfortable
        // zone, no formula applies.
        let apparent = apparent_temperature_c(20.0, Some(50.0), 10.0);
        close(apparent, 20.0, "mild");
    }

    #[test]
    fn hot_and_humid_uses_heat_index() {
        // 32 °C, 70 % RH (NWS reference: 106 °F heat
        // index ≈ 41 °C from 90 °F / 70 % RH baseline;
        // at 32 °C the HI is around 37–39 °C).
        let apparent = apparent_temperature_c(32.0, Some(70.0), 5.0);
        assert!(
            apparent > 32.0 + 2.0,
            "expected heat index bump > 2 °C, got {apparent:.2}",
        );
    }

    #[test]
    fn hot_but_dry_stays_at_raw_temp() {
        // 32 °C, 20 % RH — below the 40 % threshold
        // Rothfusz is calibrated for, so we return raw.
        let apparent = apparent_temperature_c(32.0, Some(20.0), 5.0);
        close(apparent, 32.0, "hot dry");
    }

    #[test]
    fn cold_and_windy_uses_wind_chill() {
        // 0 °C, 30 km/h wind — wind chill should push
        // the apparent temp several degrees below 0.
        let apparent = apparent_temperature_c(0.0, Some(50.0), 30.0);
        assert!(
            apparent < -3.0,
            "expected wind chill bump < -3 °C, got {apparent:.2}",
        );
    }

    #[test]
    fn cold_but_calm_stays_at_raw_temp() {
        // Cold but wind below threshold: no wind chill.
        let apparent = apparent_temperature_c(0.0, Some(50.0), 2.0);
        close(apparent, 0.0, "cold calm");
    }

    #[test]
    fn humidity_missing_skips_heat_index_branch() {
        // 32 °C, unknown humidity: heat-index branch
        // needs the humidity input so we can't run it.
        // Return raw.
        let apparent = apparent_temperature_c(32.0, None, 5.0);
        close(apparent, 32.0, "hot unknown rh");
    }

    #[test]
    fn nan_temperature_propagates_as_nan() {
        // Caller precondition is finite temp; NaN
        // inputs produce NaN outputs so a bug upstream
        // surfaces rather than being hidden.
        let apparent = apparent_temperature_c(f64::NAN, Some(50.0), 10.0);
        assert!(apparent.is_nan(), "expected NaN, got {apparent}");
    }

    #[test]
    fn nan_humidity_falls_back_to_raw_temp() {
        // NaN humidity can't feed the heat-index
        // branch; we fall through to raw.
        let apparent = apparent_temperature_c(32.0, Some(f64::NAN), 5.0);
        close(apparent, 32.0, "nan rh");
    }

    #[test]
    fn nan_wind_falls_back_to_raw_temp() {
        // NaN wind is treated as absent for the
        // wind-chill branch, same as zero or below-
        // threshold.
        let apparent = apparent_temperature_c(0.0, Some(50.0), f64::NAN);
        close(apparent, 0.0, "nan wind");
    }

    #[test]
    fn heat_index_threshold_boundary() {
        // Pin the 26.67 °C heat-index threshold. At
        // 26.66 °C with 80 % RH we return raw; at
        // 26.67 °C we start computing HI.
        let below = apparent_temperature_c(26.66, Some(80.0), 5.0);
        let at = apparent_temperature_c(26.67, Some(80.0), 5.0);
        close(below, 26.66, "below HI threshold");
        assert!(at > 26.67, "at threshold should use HI, got {at:.2}",);
    }

    #[test]
    fn wind_chill_threshold_boundary() {
        // Pin the 4.8 km/h wind-chill threshold. At
        // 4.7 km/h we return raw; at 5.0 km/h we
        // compute WC.
        let below = apparent_temperature_c(0.0, Some(50.0), 4.7);
        let at = apparent_temperature_c(0.0, Some(50.0), 5.0);
        close(below, 0.0, "below WC threshold");
        assert!(at < 0.0, "above threshold should use WC, got {at:.2}",);
    }
}
