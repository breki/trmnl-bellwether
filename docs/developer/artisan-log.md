# Artisan Findings -- Open

Code quality findings from the Artisan reviewer, newest
first. Fixed findings are moved to
[artisan-resolved.md](artisan-resolved.md).

**Next ID:** AQ-147

**Threshold:** when 10+ findings are open, a full-codebase
Artisan review is required before continuing feature work.

---

### AQ-123 — `DaySelector` numeric-overflow error message is inconsistent with string-branch
**Category:** Error quality / UX
**Logged:** 2026-04-19 (v0.17.0 review)
**Description:** `DaySelector::deserialize` accepts `Raw::Num(u8)`; a TOML value of `day = 300` fails with serde's generic "value out of range for u8" rather than the friendly `"day must be \"today\" or a non-negative integer, got 300"` format used for unknown strings. User can't tell at a glance whether the field is a widget mismatch or an out-of-range number.
**Suggested fix:** Accept `Raw::Num(i64)` (or `toml::Value`) then validate the range explicitly with the same custom error format used for the string arm.

### AQ-122 — `Config` lost `PartialEq` silently
**Category:** API / testability
**Logged:** 2026-04-19 (deferred from v0.15.0 review)
**Description:** Adding `dashboard: Option<Layout>` forced the `PartialEq` derive off `Config` because `Layout` (and its `Node`, `Canvas`, `WidgetKind`, etc.) don't derive `PartialEq`. No existing test compares two `Config` values, so no breakage — but `assert_eq!(cfg, expected)` would be a natural thing to reach for in a future test.
**Suggested fix:** Derive `PartialEq` across the layout type graph (`Layout`, `Canvas`, `Node`, `SplitNode`, `Child`, `Sizing`, `WidgetKind`) and restore `PartialEq` on `Config`. All fields are `u32`, `String`, `Vec`, or enums of the same — no exotic types to worry about.
