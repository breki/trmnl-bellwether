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
upstream tree — `strip_xml_prolog` in
`crates/bellwether/src/dashboard/icons.rs` skips the
`<?xml …?>` declaration and generator comment at read
time rather than editing the files in place, so
upstream attribution stays intact.

## License

SIL Open Font License, Version 1.1. Full text in
[`LICENSE`](LICENSE). The upstream project declares
OFL 1.1 in its [README][upstream-readme] but does not
itself ship the license text — this bundle includes
the canonical text from <https://openfontlicense.org>
with Erik Flowers' copyright header so the
SIL OFL §2 "bundled and redistributed" condition is
met when bellwether itself is redistributed.

[upstream-readme]: https://github.com/erikflowers/weather-icons#license

## Bundled files

| File | WMO coarse category it serves |
|------|------------------------------|
| `wi-day-sunny.svg` | Clear |
| `wi-day-cloudy.svg` | PartlyCloudy |
| `wi-cloudy.svg` | Cloudy |
| `wi-rain.svg` | Rain |

The full two-tier fidelity model (9 category icons
plus ~15 detailed WMO-specific icons) is sketched in
`docs/developer/HANDOFF.md` and will land
incrementally; additions to this directory should
come from the same upstream `svg/` tree to preserve
the License condition.
