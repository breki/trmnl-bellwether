# Bundled fonts

## SourceSans3-Semibold.ttf

Adobe's Source Sans 3 in the Semibold weight (600). A
humanist sans-serif with a dotted (non-slashed) zero,
open apertures, and strokes heavy enough to survive
Floyd-Steinberg dithering to 1-bit e-ink. Legible at
both display sizes (~180 px for the current temperature)
and label sizes (~28-36 px for wind and day labels).

- **Author**: Adobe / Paul D. Hunt
- **Source**:
  <https://github.com/adobe-fonts/source-sans>
  (Google Fonts mirror:
  <https://fonts.google.com/specimen/Source+Sans+3>)
- **Licence**: SIL Open Font License 1.1. Free for
  any use including commercial, with source font
  redistribution permitted but font sales prohibited.

We bundle this font into the `bellwether` crate via
`include_bytes!` so the dashboard renders consistently
across all deployment targets regardless of the host's
installed fonts. Unlike a pixel font, Source Sans is
authored with vector outlines, so any font-size
renders crisply — no grid-alignment constraints on
the layout.

Attribution appears in the crate's `Cargo.toml`
metadata and should be included in any public-facing
documentation that mentions the dashboard's
typography.
