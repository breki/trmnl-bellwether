# Weather Icons (bundled SVG subset)

These SVG files are a verbatim subset of the
[Weather Icons][wi] project by Erik Flowers, used by
the bellwether dashboard renderer for its weather
condition glyphs.

[wi]: https://erikflowers.github.io/weather-icons/

## Source

Downloaded verbatim from
`https://raw.githubusercontent.com/erikflowers/weather-icons/master/svg/`
on 2026-04-20. The files are byte-identical to the
upstream tree and remain so — `skip_to_svg_root` in
`crates/bellwether/src/dashboard/icons.rs` steps past
the XML prolog + generator comments at read time
rather than editing the files in place.

Byte-identity is **enforced** by
`tests::bundled_icons_match_pinned_sha256` in
`icons.rs`, which compares each file's SHA-256 against
a pin committed in the test module. Any accidental
whitespace-normalising hook or hand-edit fails the
build loudly.

## License

SIL Open Font License, Version 1.1. Full text in
[`LICENSE`](LICENSE). The upstream project declares
OFL 1.1 in its [README][upstream-readme] but does not
itself ship a signed copyright statement — this bundle
reproduces the canonical OFL text from
<https://openfontlicense.org> alongside a softened
header that acknowledges the upstream gap.

[upstream-readme]: https://github.com/erikflowers/weather-icons#license

Source-distribution §2 compliance is satisfied by this
`LICENSE` file being distributed next to the SVGs in
the repository. **Binary-distribution §2 compliance**
is satisfied by
[`bellwether::licenses::WEATHER_ICONS_OFL`][licenses]
and the `/licenses` endpoint on `bellwether-web`,
which serves the same text via HTTP as a "machine-
readable metadata field … easily viewed by the user"
per §2.

[licenses]: ../../../src/licenses.rs

## Modifications (OFL §5 — Reserved Font Name)

Upstream uses the name "Weather Icons" as its primary
font name. Under OFL §5, **no Modified Version** of
the Font Software may use the upstream's primary name.
If you edit any of these SVG files (recolour, add
strokes, change paths, or repath the `fill` default),
the resulting file is a Modified Version and the
"Weather Icons" identifier (including the `id="Layer_1"`
and any Illustrator metadata that references the name)
must be dropped or renamed before redistribution.

**Prefer re-downloading from upstream over editing
in place.** The pinned SHA-256 check in `icons.rs`
tests exists partly to enforce this — an in-place
edit fails the build before it reaches a release.

## Bundled files

The 9 category icons land as the PR 3 baseline — one
SVG per [`ConditionCategory`] variant. Detailed
per-[`WmoCode`] glyphs land incrementally on top of
this set in PR 4+ and are reached only when a
`weather-icon` widget opts into `fidelity = "detailed"`.

### Category icons (Simple fidelity)

| File | `ConditionCategory` it serves |
|------|-------------------------------|
| `wi-day-sunny.svg`     | `Clear` |
| `wi-day-cloudy.svg`    | `PartlyCloudy` |
| `wi-cloudy.svg`        | `Cloudy` |
| `wi-fog.svg`           | `Fog` |
| `wi-sprinkle.svg`      | `Drizzle` |
| `wi-rain.svg`          | `Rain` |
| `wi-snow.svg`          | `Snow` |
| `wi-thunderstorm.svg`  | `Thunderstorm` |
| `wi-na.svg`            | `Unknown` |

### Detailed-fidelity icons (per `WmoCode`)

| File | `WmoCode` it serves | Coarse fallback |
|------|---------------------|-----------------|
| `wi-hail.svg` | `ThunderstormHailHeavy` | `wi-thunderstorm.svg` |
| `wi-snow-wind.svg` | `SnowHeavy` | `wi-snow.svg` |
| `wi-rain-wind.svg` | `RainHeavy` | `wi-rain.svg` |
| `wi-sleet.svg` | `FreezingRainHeavy` | `wi-rain.svg` |

[`ConditionCategory`]: ../../../src/dashboard/classify.rs
[`WmoCode`]: ../../../src/dashboard/classify.rs

Additions to this directory must come from the same
upstream `svg/` tree (with a fresh SHA-256 pin in
`icons.rs`) to preserve both the license condition
and the byte-identity claim.
