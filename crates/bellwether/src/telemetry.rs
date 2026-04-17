//! Device telemetry — data the TRMNL device posts to
//! the bellwether server via `/api/log` every time it
//! wakes up.
//!
//! The types live here (rather than in `publish` or
//! `dashboard`) so both those modules can depend on
//! them without forming a cycle:
//!
//! - `publish::ImageSink::latest_telemetry` returns a
//!   [`DeviceTelemetry`] snapshot.
//! - `dashboard::model::ModelContext` embeds one so
//!   the SVG builder can render the battery
//!   indicator.
//! - `bellwether-web::api::trmnl::TrmnlState` stores
//!   one and is the authoritative writer.
//!
//! Today only `battery_voltage` is populated. PR 3e
//! tracks additional fields (RSSI, firmware version)
//! — they'll land here as they're wired into the
//! `/api/log` handler.

/// Latest telemetry the bellwether server has
/// received from the TRMNL device.
///
/// All fields are `Option<_>` because the device
/// doesn't always populate every field on every post
/// (TRMNL firmware posts keepalives, error reports,
/// and wake-up reports with different shapes) and
/// because the bellwether process may also run with
/// no real device attached (`--dev` mode, local
/// testing). Callers render a placeholder for any
/// `None` field rather than inventing a default — the
/// dashboard has a consistent "never show fake
/// numbers" convention.
///
/// Not `#[non_exhaustive]` so downstream code can
/// pattern-match by name; further fields are expected
/// as PR 3e adds RSSI and firmware version.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DeviceTelemetry {
    /// Most recent battery voltage reported by the
    /// device, in volts. `None` if no device has
    /// posted yet, or if the most recent post with a
    /// battery field is older than any merge policy
    /// the caller enforces.
    pub battery_voltage: Option<f64>,
}

impl DeviceTelemetry {
    /// Merge fresh fields from `update` into `self`,
    /// keeping any prior values whose corresponding
    /// field in `update` is `None`.
    ///
    /// The TRMNL device posts `/api/log` for many
    /// reasons — wake-up reports, error reports,
    /// keepalives. Some posts include
    /// `battery_voltage`, some don't. A "last
    /// reported overwrite" policy would wipe the
    /// battery indicator every time a keepalive
    /// arrives; this merge policy keeps the most
    /// recently-reported value for each field until
    /// a fresher value lands.
    pub fn merge_from(&mut self, update: DeviceTelemetry) {
        if update.battery_voltage.is_some() {
            self.battery_voltage = update.battery_voltage;
        }
    }
}

/// `LiPo` cell voltage that maps to 0 % remaining
/// charge under the linear approximation this module
/// uses. TRMNL devices start cut-off protection at
/// ~3.0 V; 3.3 V is the effective "low" battery
/// point.
const BATTERY_EMPTY_V: f64 = 3.3;

/// `LiPo` cell voltage that maps to 100 % remaining
/// charge. Above this the cell is at full charge or
/// the charger is still active and we clamp.
const BATTERY_FULL_V: f64 = 4.2;

/// Map a `LiPo` battery voltage to an integer
/// percentage in `0..=100` using a simple linear
/// approximation between [`BATTERY_EMPTY_V`] and
/// [`BATTERY_FULL_V`].
///
/// `LiPo` discharge is non-linear, so this is an
/// approximation suitable for a 3-pixel-tall status
/// indicator, not a precise fuel gauge. Good enough
/// to distinguish "mostly full" from "almost empty"
/// at a glance.
#[must_use]
pub fn battery_voltage_to_pct(voltage: f64) -> Option<u8> {
    if !voltage.is_finite() {
        return None;
    }
    let span = BATTERY_FULL_V - BATTERY_EMPTY_V;
    let raw = ((voltage - BATTERY_EMPTY_V) / span).clamp(0.0, 1.0) * 100.0;
    // The clamp + multiplication keep `raw` in
    // `[0.0, 100.0]`, which fits in u8 via truncation.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let pct = raw.round() as u8;
    Some(pct)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_voltage_to_pct_at_full_charge_is_100() {
        assert_eq!(battery_voltage_to_pct(4.2), Some(100));
        assert_eq!(battery_voltage_to_pct(4.3), Some(100));
    }

    #[test]
    fn battery_voltage_to_pct_at_empty_is_0() {
        assert_eq!(battery_voltage_to_pct(3.3), Some(0));
        assert_eq!(battery_voltage_to_pct(3.0), Some(0));
    }

    #[test]
    fn battery_voltage_to_pct_midpoint() {
        assert_eq!(battery_voltage_to_pct(3.75), Some(50));
    }

    #[test]
    fn battery_voltage_to_pct_rejects_non_finite() {
        assert_eq!(battery_voltage_to_pct(f64::NAN), None);
        assert_eq!(battery_voltage_to_pct(f64::INFINITY), None);
        assert_eq!(battery_voltage_to_pct(f64::NEG_INFINITY), None);
    }

    #[test]
    fn device_telemetry_default_is_all_none() {
        let t = DeviceTelemetry::default();
        assert_eq!(t.battery_voltage, None);
    }

    #[test]
    fn merge_from_keeps_prior_value_when_update_is_none() {
        // Keepalive-style post without a battery
        // reading must not wipe the cached value.
        let mut cached = DeviceTelemetry {
            battery_voltage: Some(3.9),
        };
        cached.merge_from(DeviceTelemetry {
            battery_voltage: None,
        });
        assert_eq!(cached.battery_voltage, Some(3.9));
    }

    #[test]
    fn merge_from_replaces_when_update_has_fresh_value() {
        let mut cached = DeviceTelemetry {
            battery_voltage: Some(3.9),
        };
        cached.merge_from(DeviceTelemetry {
            battery_voltage: Some(4.05),
        });
        assert_eq!(cached.battery_voltage, Some(4.05));
    }
}
