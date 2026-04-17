# Bundled fonts

## AtkinsonHyperlegible-Regular.ttf

A proportional sans-serif designed by the Braille
Institute of America for maximum legibility at any
resolution, with distinctive letterforms that reduce
character confusion (e.g. the bowl of the `a` vs the
opening of the `e`; the angles on `I`, `l`, `1`).
Particularly well-suited to 1-bit e-ink rendering
after Floyd-Steinberg dithering — the heavier strokes
and wider apertures survive the dither pattern better
than a thin geometric sans would.

- **Author**: Braille Institute of America / Applied
  Design Works
- **Source**: <https://brailleinstitute.org/freefont>
  (GitHub mirror:
  <https://github.com/googlefonts/atkinson-hyperlegible>)
- **Licence**: SIL Open Font License 1.1. Free for
  any use including commercial, with source font
  redistribution permitted but font sales prohibited.
  Full licence text at the Braille Institute page.

We bundle this font into the `bellwether` crate via
`include_bytes!` so the dashboard renders consistently
across all deployment targets regardless of the host's
installed fonts. Unlike a pixel font, Atkinson is
authored with vector outlines, so any font-size
renders crisply — no grid-alignment constraints on
the layout.

Attribution appears in the crate's `Cargo.toml`
metadata and should be included in any public-facing
documentation that mentions the dashboard's
typography.
