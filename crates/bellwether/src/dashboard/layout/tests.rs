use super::*;

fn canvas(w: u32, h: u32) -> Rect {
    Rect { x: 0, y: 0, w, h }
}

fn widget(kind: WidgetKind) -> Node {
    Node::Widget(kind)
}

fn widgets_of(resolved: &Resolved<'_>) -> Vec<Rect> {
    resolved.widgets.iter().map(|p| p.bounds).collect()
}

#[test]
fn single_widget_fills_canvas() {
    let root = widget(WidgetKind::Clock);
    let resolved = resolve_node(&root, canvas(800, 480)).unwrap();
    assert_eq!(resolved.widgets.len(), 1);
    assert_eq!(
        resolved.widgets[0].bounds,
        Rect {
            x: 0,
            y: 0,
            w: 800,
            h: 480,
        }
    );
    assert!(matches!(resolved.widgets[0].widget, WidgetKind::Clock));
    assert!(resolved.dividers.is_empty());
}

#[test]
fn horizontal_split_two_fixed_children() {
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: false,
        gap: 0,
        children: vec![
            Child::fixed(200, widget(WidgetKind::Brand)),
            Child::fixed(600, widget(WidgetKind::Clock)),
        ],
    });
    let resolved = resolve_node(&root, canvas(800, 480)).unwrap();
    assert_eq!(
        widgets_of(&resolved),
        vec![
            Rect {
                x: 0,
                y: 0,
                w: 200,
                h: 480
            },
            Rect {
                x: 200,
                y: 0,
                w: 600,
                h: 480
            },
        ]
    );
}

#[test]
fn vertical_split_flex_children_split_evenly() {
    let root = Node::Split(SplitNode {
        split: Direction::Vertical,
        divider: false,
        gap: 0,
        children: vec![
            Child::flex(1, widget(WidgetKind::Brand)),
            Child::flex(1, widget(WidgetKind::Clock)),
        ],
    });
    let resolved = resolve_node(&root, canvas(800, 480)).unwrap();
    assert_eq!(
        widgets_of(&resolved),
        vec![
            Rect {
                x: 0,
                y: 0,
                w: 800,
                h: 240
            },
            Rect {
                x: 0,
                y: 240,
                w: 800,
                h: 240
            },
        ]
    );
}

#[test]
fn mixed_fixed_and_flex_fixed_taken_off_first() {
    let root = Node::Split(SplitNode {
        split: Direction::Vertical,
        divider: false,
        gap: 0,
        children: vec![
            Child::fixed(50, widget(WidgetKind::Brand)),
            Child::flex(1, widget(WidgetKind::Clock)),
            Child::fixed(50, widget(WidgetKind::Battery)),
        ],
    });
    let resolved = resolve_node(&root, canvas(800, 480)).unwrap();
    let rects = widgets_of(&resolved);
    assert_eq!(rects[0].h, 50);
    assert_eq!(
        rects[1],
        Rect {
            x: 0,
            y: 50,
            w: 800,
            h: 380
        }
    );
    assert_eq!(
        rects[2],
        Rect {
            x: 0,
            y: 430,
            w: 800,
            h: 50
        }
    );
}

#[test]
fn flex_weights_split_remaining_proportionally() {
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: false,
        gap: 0,
        children: vec![
            Child::flex(2, widget(WidgetKind::Brand)),
            Child::flex(1, widget(WidgetKind::Clock)),
            Child::flex(3, widget(WidgetKind::Battery)),
        ],
    });
    let resolved = resolve_node(&root, canvas(900, 100)).unwrap();
    let rects = widgets_of(&resolved);
    assert_eq!(rects[0].w, 300);
    assert_eq!(rects[1].w, 150);
    assert_eq!(rects[2].w, 450);
    assert_eq!(rects[0].x, 0);
    assert_eq!(rects[1].x, 300);
    assert_eq!(rects[2].x, 450);
}

#[test]
fn flex_rounding_remainder_goes_to_last_flex_child() {
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: false,
        gap: 0,
        children: vec![
            Child::flex(1, widget(WidgetKind::Brand)),
            Child::flex(1, widget(WidgetKind::Clock)),
            Child::flex(1, widget(WidgetKind::Battery)),
        ],
    });
    let resolved = resolve_node(&root, canvas(10, 10)).unwrap();
    let rects = widgets_of(&resolved);
    assert_eq!(rects[0].w, 3);
    assert_eq!(rects[1].w, 3);
    assert_eq!(rects[2].w, 4);
    assert_eq!(rects[0].w + rects[1].w + rects[2].w, 10);
}

#[test]
fn nested_splits() {
    let root = Node::Split(SplitNode {
        split: Direction::Vertical,
        divider: false,
        gap: 0,
        children: vec![
            Child::fixed(
                50,
                Node::Split(SplitNode {
                    split: Direction::Horizontal,
                    divider: false,
                    gap: 0,
                    children: vec![
                        Child::fixed(200, widget(WidgetKind::Brand)),
                        Child::flex(1, widget(WidgetKind::Clock)),
                    ],
                }),
            ),
            Child::flex(1, widget(WidgetKind::TempNow { day: TodayOnly })),
        ],
    });
    let resolved = resolve_node(&root, canvas(800, 480)).unwrap();
    assert_eq!(
        widgets_of(&resolved),
        vec![
            Rect {
                x: 0,
                y: 0,
                w: 200,
                h: 50
            },
            Rect {
                x: 200,
                y: 0,
                w: 600,
                h: 50
            },
            Rect {
                x: 0,
                y: 50,
                w: 800,
                h: 430
            },
        ]
    );
}

#[test]
fn divider_reserves_space_and_emits_placement() {
    // Two flex-1 children in a 102-px horizontal split
    // with a divider: 2 px consumed, 100 remains → 50
    // each. Divider is emitted as a placement.
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: true,
        gap: 0,
        children: vec![
            Child::flex(1, widget(WidgetKind::Brand)),
            Child::flex(1, widget(WidgetKind::Clock)),
        ],
    });
    let resolved = resolve_node(&root, canvas(102, 50)).unwrap();
    assert_eq!(
        widgets_of(&resolved),
        vec![
            Rect {
                x: 0,
                y: 0,
                w: 50,
                h: 50
            },
            Rect {
                x: 52,
                y: 0,
                w: 50,
                h: 50
            },
        ]
    );
    assert_eq!(resolved.dividers.len(), 1);
    assert_eq!(
        resolved.dividers[0].bounds,
        Rect {
            x: 50,
            y: 0,
            w: 2,
            h: 50
        }
    );
    assert_eq!(resolved.dividers[0].orientation, Direction::Horizontal);
}

#[test]
fn vertical_split_dividers_are_horizontal_strips() {
    let root = Node::Split(SplitNode {
        split: Direction::Vertical,
        divider: true,
        gap: 0,
        children: vec![
            Child::fixed(50, widget(WidgetKind::Brand)),
            Child::fixed(50, widget(WidgetKind::Clock)),
        ],
    });
    let resolved = resolve_node(&root, canvas(800, 102)).unwrap();
    assert_eq!(resolved.dividers.len(), 1);
    assert_eq!(
        resolved.dividers[0].bounds,
        Rect {
            x: 0,
            y: 50,
            w: 800,
            h: 2
        }
    );
    assert_eq!(resolved.dividers[0].orientation, Direction::Vertical);
}

#[test]
fn gap_adds_whitespace_between_children() {
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: false,
        gap: 10,
        children: vec![
            Child::flex(1, widget(WidgetKind::Brand)),
            Child::flex(1, widget(WidgetKind::Clock)),
        ],
    });
    let resolved = resolve_node(&root, canvas(100, 50)).unwrap();
    let rects = widgets_of(&resolved);
    assert_eq!(rects[0].w, 45);
    assert_eq!(rects[1].x, 55);
    assert_eq!(rects[1].w, 45);
}

#[test]
fn overflow_error_when_fixed_exceeds_parent() {
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: false,
        gap: 0,
        children: vec![
            Child::fixed(500, widget(WidgetKind::Brand)),
            Child::fixed(500, widget(WidgetKind::Clock)),
        ],
    });
    let err = resolve_node(&root, canvas(800, 100)).unwrap_err();
    assert_eq!(
        err,
        LayoutError::Overflow {
            fixed: 1000,
            sep: 0,
            available: 800,
        }
    );
}

#[test]
fn empty_split_errors() {
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: false,
        gap: 0,
        children: vec![],
    });
    let err = resolve_node(&root, canvas(100, 100)).unwrap_err();
    assert_eq!(err, LayoutError::EmptySplit);
}

#[test]
fn huge_flex_weight_overflows_without_panicking() {
    // flex_budget * weight would overflow u32 here.
    // Resolver must detect and return an error rather
    // than wrapping silently.
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: false,
        gap: 0,
        children: vec![
            Child::flex(u32::MAX, widget(WidgetKind::Brand)),
            Child::flex(u32::MAX, widget(WidgetKind::Clock)),
        ],
    });
    // u64 math handles this fine since weights fit in
    // u64 and flex_budget * weight stays below 2^64.
    // So it should succeed, not error — assert it
    // produces sane widths.
    let resolved = resolve_node(&root, canvas(800, 100)).unwrap();
    let rects = widgets_of(&resolved);
    assert_eq!(rects[0].w + rects[1].w, 800);
}

#[test]
fn huge_gap_overflow_errors() {
    let root = Node::Split(SplitNode {
        split: Direction::Horizontal,
        divider: false,
        gap: u32::MAX,
        children: vec![
            Child::fixed(10, widget(WidgetKind::Brand)),
            Child::fixed(10, widget(WidgetKind::Clock)),
        ],
    });
    let err = resolve_node(&root, canvas(800, 100)).unwrap_err();
    assert!(matches!(err, LayoutError::Overflow { .. }));
}

#[test]
fn parses_layout_toml() {
    let toml_src = r#"
canvas = { width = 800, height = 480 }

split = "vertical"
divider = true

[[children]]
size = 50
split = "horizontal"
children = [
  { size = 200, widget = "brand" },
  { flex = 1,   widget = "header-title", text = "Weather Report" },
  { size = 150, widget = "clock" },
  { size = 100, widget = "battery" },
]

[[children]]
size = 140
widget = "temp-now"

[[children]]
flex = 1
split = "horizontal"
children = [
  { flex = 1, widget = "weather-icon", day = 0 },
  { flex = 1, widget = "weather-icon", day = 1 },
  { flex = 1, widget = "weather-icon", day = 2 },
]
"#;
    let layout: Layout = toml::from_str(toml_src).unwrap();
    assert_eq!(layout.canvas.width, 800);
    assert_eq!(layout.canvas.height, 480);

    let resolved = layout.resolve().unwrap();
    assert_eq!(resolved.widgets.len(), 8);
    // Top-level vertical split has divider=true, so two
    // horizontal divider strips between 3 children.
    assert_eq!(resolved.dividers.len(), 2);
    for d in &resolved.dividers {
        assert_eq!(d.orientation, Direction::Vertical);
        assert_eq!(d.bounds.h, 2);
    }

    match resolved.widgets[1].widget {
        WidgetKind::HeaderTitle { text } => {
            assert_eq!(text, "Weather Report");
        }
        other => panic!("expected HeaderTitle, got {other:?}"),
    }
    for (i, idx) in [5usize, 6, 7].iter().enumerate() {
        match resolved.widgets[*idx].widget {
            WidgetKind::WeatherIcon { day, .. } => match day {
                DaySelector::Offset(n) => assert_eq!(usize::from(*n), i),
                DaySelector::Today => panic!("expected numeric day offset"),
            },
            other => panic!("expected WeatherIcon, got {other:?}"),
        }
    }
}

// Child validation happens at try_from time, but when
// the Child sits inside `Node` (serde untagged) serde
// swallows the specific error into "did not match any
// variant". Testing the Child type directly keeps the
// invariant visible.

#[test]
fn child_with_neither_size_nor_flex_fails_to_parse() {
    let err = toml::from_str::<Child>(r#"widget = "brand""#).unwrap_err();
    assert!(
        err.to_string()
            .contains("must have either `size` or `flex`"),
        "unexpected error: {err}"
    );
}

#[test]
fn child_with_both_size_and_flex_fails_to_parse() {
    let err = toml::from_str::<Child>(
        r#"size = 10
flex = 1
widget = "brand"
"#,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("cannot have both"),
        "unexpected error: {err}"
    );
}

#[test]
fn child_with_flex_zero_fails_to_parse() {
    let err = toml::from_str::<Child>(
        r#"flex = 0
widget = "brand"
"#,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("must be at least 1"),
        "unexpected error: {err}"
    );
}

#[test]
fn day_selector_accepts_today_string_and_offset_number() {
    let today: Child = toml::from_str(
        r#"size = 10
widget = "weather-icon"
day = "today"
"#,
    )
    .unwrap();
    match today.node {
        Node::Widget(WidgetKind::WeatherIcon {
            day: DaySelector::Today,
            ..
        }) => {}
        other => panic!("expected WeatherIcon{{day=Today}}, got {other:?}"),
    }
    let offset: Child = toml::from_str(
        r#"size = 10
widget = "day-name"
day = 2
"#,
    )
    .unwrap();
    match offset.node {
        Node::Widget(WidgetKind::DayName {
            day: DaySelector::Offset(2),
        }) => {}
        other => panic!("expected DayName{{day=Offset(2)}}, got {other:?}"),
    }
}

#[test]
fn day_selector_rejects_unknown_string() {
    // Tested directly rather than through the untagged
    // `Node` enum because serde's untagged dispatch
    // discards inner error messages.
    #[derive(Debug, Deserialize)]
    struct Holder {
        #[allow(dead_code)]
        day: DaySelector,
    }
    let err = toml::from_str::<Holder>(r#"day = "tomorrow""#).unwrap_err();
    assert!(
        err.to_string().contains("day must be"),
        "unexpected error: {err}"
    );
}

#[test]
fn today_selector_case_insensitive() {
    #[derive(Debug, Deserialize)]
    struct Holder {
        #[allow(dead_code)]
        day: DaySelector,
    }
    for casing in ["today", "Today", "TODAY", "tOdAy"] {
        let h: Holder =
            toml::from_str(&format!(r#"day = "{casing}""#)).unwrap();
        assert!(
            matches!(h.day, DaySelector::Today),
            "casing {casing:?} didn't resolve to Today",
        );
    }
}

#[test]
fn today_only_rejects_numeric_day_with_pointed_message() {
    // Tested directly (not through the untagged `Node`
    // enum) because serde's untagged dispatch discards
    // inner error messages.
    #[derive(Debug, Deserialize)]
    struct Holder {
        #[allow(dead_code)]
        day: TodayOnly,
    }
    let err = toml::from_str::<Holder>("day = 2").unwrap_err();
    assert!(
        err.to_string().contains("today-only"),
        "unexpected error: {err}"
    );
    // Plain absence parses via `#[serde(default)]` —
    // covered implicitly by `parses_layout_toml` and
    // the `TempNow { day: TodayOnly }` construction
    // in `nested_splits`.
    let ok: Holder = toml::from_str("day = \"today\"").unwrap();
    assert_eq!(ok.day, TodayOnly);
}

#[test]
fn temp_high_accepts_optional_label() {
    let labelled: Child = toml::from_str(
        r#"size = 10
widget = "temp-high"
day = "today"
label = "H"
"#,
    )
    .unwrap();
    match labelled.node {
        Node::Widget(WidgetKind::TempHigh { day, label }) => {
            assert!(matches!(day, DaySelector::Today));
            assert_eq!(label.as_deref(), Some("H"));
        }
        other => panic!("expected TempHigh, got {other:?}"),
    }
    let bare: Child = toml::from_str(
        r#"size = 10
widget = "temp-low"
day = 1
"#,
    )
    .unwrap();
    match bare.node {
        Node::Widget(WidgetKind::TempLow { day, label }) => {
            assert!(matches!(day, DaySelector::Offset(1)));
            assert!(label.is_none());
        }
        other => panic!("expected TempLow, got {other:?}"),
    }
}
