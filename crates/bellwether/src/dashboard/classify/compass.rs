//! Compass-direction bucketing.
//!
//! Pure function that translates a wind direction in
//! compass degrees (clockwise from North) into one of
//! eight cardinal octants, for the dashboard's wind
//! label. Lives in its own sibling to [`super::weather`]
//! because the two share no types, invariants, or
//! tests; they coexisted in one file only because both
//! are "display-layer bucketing".

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

#[cfg(test)]
mod tests {
    use super::*;

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
