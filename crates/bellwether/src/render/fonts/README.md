# Bundled fonts

## m6x11plus.ttf

A 6×11 proportional pixel font with extended Latin
character coverage (Dutch, French, German, Italian,
Norwegian, Polish, Portuguese, Spanish, Swedish).

- **Author**: Daniel Linssen (managore)
- **Source**: <https://managore.itch.io/m6x11>
- **Licence**: free to use with attribution (per the
  itch.io page "free with attribution!")

We bundle this font into the `bellwether` crate via
`include_bytes!` so the dashboard renders consistent
text on every deployment target regardless of the
host's installed fonts. Recommended point sizes
(per the author's notes): 18, 36, 54 — i.e. integer
multiples of 18.

Attribution appears in the crate's
`Cargo.toml` authors/credits and should be included
in any public-facing documentation that mentions the
dashboard's typography.
