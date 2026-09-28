//! The HUD chrome: the top bar and its scanlines, the map-room
//! backdrop, framed panels and modals, and the VS matchup splitter.

use super::*;
// Explicit: a macro reached only through the glob above is ambiguous.
use sweeten::widget::row;

/// Rotate a color's hue by `deg` degrees (HSV space; saturation and
/// value hold). This is how accent-relative companion tones are
/// derived — e.g. the scanline's far stop sits a quarter-turn from
/// the accent so the pair reads as one energy family no matter
/// which chrome color the user picked.
pub fn rotate_hue(c: iced::Color, deg: f32) -> iced::Color {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == c.r {
        60.0 * ((c.g - c.b) / d).rem_euclid(6.0)
    } else if max == c.g {
        60.0 * ((c.b - c.r) / d + 2.0)
    } else {
        60.0 * ((c.r - c.g) / d + 4.0)
    };
    let h = (h + deg).rem_euclid(360.0);
    let (s, v) = (if max == 0.0 { 0.0 } else { d / max }, max);
    let chroma = v * s;
    let x = chroma * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - chroma;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    iced::Color {
        r: r + m,
        g: g + m,
        b: b + m,
        a: c.a,
    }
}

/// Top nav strip background. Vertical gradient (lighter top, darker
/// bottom) so it reads as a console plate catching overhead light
/// rather than a flat sheet of pixels. Drops a soft shadow onto
/// the body surface below so the seam between HUD and content
/// feels lifted, not stamped. The accent scanline is rendered as
/// a separate row underneath; this style intentionally has no
/// bottom border so the two layers don't fight.
pub fn hud_bar(theme: &Theme) -> iced::widget::container::Style {
    if tango_ui::style::is_advance_wars(theme) {
        use tango_ui::style::aw;
        return iced::widget::container::Style {
            background: Some(iced::Background::Gradient(iced::Gradient::Linear(
                iced::gradient::Linear::new(0.0)
                    .add_stop(0.0, aw::PLATE_TOP)
                    .add_stop(1.0, aw::PLATE_BOTTOM),
            ))),
            text_color: Some(aw::PLATE_TEXT),
            shadow: iced::Shadow {
                color: iced::Color {
                    a: 0.45,
                    ..iced::Color::BLACK
                },
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        };
    }
    let p = theme.extended_palette();
    let bg = theme.palette().background;
    let text = theme.palette().text;
    let (top, bottom) = if p.is_dark {
        // Pull toward black at the bottom; the top stays close to
        // the bg color so the gradient is felt, not seen. Uniform
        // channel decay — the old blue-retaining multipliers were
        // a navy-era trick that re-tints a neutral base cool.
        (
            iced::Color {
                r: bg.r * 0.7,
                g: bg.g * 0.7,
                b: bg.b * 0.7,
                a: 1.0,
            },
            iced::Color {
                r: bg.r * 0.4,
                g: bg.g * 0.4,
                b: bg.b * 0.4,
                a: 1.0,
            },
        )
    } else {
        // Light theme: subtle parchment gradient — top slightly
        // tinted toward text, bottom slightly more so.
        (mix(bg, text, 0.05), mix(bg, text, 0.12))
    };
    iced::widget::container::Style {
        background: Some(iced::Background::Gradient(iced::Gradient::Linear(
            iced::gradient::Linear::new(0.0)
                .add_stop(0.0, top)
                .add_stop(1.0, bottom),
        ))),
        text_color: Some(text),
        shadow: iced::Shadow {
            color: iced::Color {
                a: if p.is_dark { 0.45 } else { 0.18 },
                ..iced::Color::BLACK
            },
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..Default::default()
    }
}

/// Body surface (everything below the HUD bar). Paints no
/// background of its own — the content layer rides on
/// [`map_backdrop`], stacked underneath by `App::view`, and an
/// opaque fill here would blot the map grid out.
pub fn body_surface(theme: &Theme) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: None,
        text_color: Some(theme.palette().text),
        ..Default::default()
    }
}

/// The map-room backdrop behind the tabs, welcome and results
/// screens: a deep navy wash with a faint square grid, like a
/// campaign map pinned to a war-room table. Every fourth line is a
/// touch stronger so the grid reads as sectors rather than graph
/// paper. Static and cached; the geometry only re-tessellates when
/// the canvas resizes or the theme flips.
pub fn map_backdrop<'a, M: 'a>() -> Element<'a, M> {
    use iced::widget::canvas::{self, gradient, Canvas, Path, Stroke, Style};
    use iced::{Point, Rectangle, Renderer};

    /// Grid cell size in logical pixels.
    const CELL: f32 = 32.0;
    /// Every `MAJOR`th line is a sector line.
    const MAJOR: i32 = 4;

    struct Backdrop;

    #[derive(Default)]
    struct State {
        cache: canvas::Cache,
        /// Palette fingerprint the cached geometry was drawn with.
        /// `Cache` only invalidates on size changes, so theme flips
        /// have to clear it by hand or the old colors stick.
        key: std::cell::Cell<u64>,
    }

    impl<M> canvas::Program<M> for Backdrop {
        type State = State;

        fn draw(
            &self,
            state: &State,
            renderer: &Renderer,
            theme: &Theme,
            bounds: Rectangle,
            _cursor: iced::mouse::Cursor,
        ) -> Vec<canvas::Geometry> {
            let bg = theme.palette().background;
            let text = theme.palette().text;
            let dark = theme.extended_palette().is_dark;
            let fp = |c: iced::Color| {
                (((c.r * 255.0) as u64) << 16) | (((c.g * 255.0) as u64) << 8) | ((c.b * 255.0) as u64)
            };
            let key = fp(bg) | (fp(text) << 24) | ((dark as u64) << 63);
            if state.key.replace(key) != key {
                state.cache.clear();
            }
            let field = tango_ui::style::is_advance_wars(theme);
            let geom = state.cache.draw(renderer, bounds.size(), |frame| {
                let w = frame.width();
                let h = frame.height();
                if field {
                    field_scene(frame);
                    return;
                }
                // Map-table navy on dark; a pale blue-grey chart
                // paper on light.
                let navy = iced::Color::from_rgb8(0x10, 0x1c, 0x33);
                let (top, bottom) = if dark {
                    (mix(bg, navy, 0.85), mix(mix(bg, navy, 0.6), iced::Color::BLACK, 0.25))
                } else {
                    let chart = iced::Color::from_rgb8(0xdc, 0xe3, 0xec);
                    (mix(bg, chart, 0.55), mix(bg, chart, 0.85))
                };
                frame.fill_rectangle(
                    Point::ORIGIN,
                    frame.size(),
                    gradient::Linear::new(Point::ORIGIN, Point::new(0.0, h))
                        .add_stop(0.0, top)
                        .add_stop(1.0, bottom),
                );

                let ink = if dark {
                    iced::Color::from_rgb8(0x8f, 0xb4, 0xe8)
                } else {
                    iced::Color::from_rgb8(0x2a, 0x45, 0x6e)
                };
                let (minor_a, major_a) = if dark { (0.05, 0.10) } else { (0.06, 0.12) };
                let line = |frame: &mut canvas::Frame, from: Point, to: Point, major: bool| {
                    frame.stroke(
                        &Path::line(from, to),
                        Stroke {
                            style: Style::Solid(iced::Color {
                                a: if major { major_a } else { minor_a },
                                ..ink
                            }),
                            width: 1.0,
                            ..Stroke::default()
                        },
                    );
                };
                // Lines sit on half pixels so the 1 px strokes stay crisp.
                let columns = (w / CELL).ceil() as i32;
                for i in 1..=columns {
                    let x = (i as f32 * CELL).floor() + 0.5;
                    line(frame, Point::new(x, 0.0), Point::new(x, h), i % MAJOR == 0);
                }
                let rows = (h / CELL).ceil() as i32;
                for i in 1..=rows {
                    let y = (i as f32 * CELL).floor() + 0.5;
                    line(frame, Point::new(0.0, y), Point::new(w, y), i % MAJOR == 0);
                }
            });
            vec![geom]
        }
    }

    Canvas::new(Backdrop).width(Length::Fill).height(Length::Fill).into()
}

/// The Advance Wars backdrop: a sepia field under a hazy sky, two
/// ranges of hills, and a faint map grid over it all, like a battle
/// map laid over the land it charts. Drawn, so it scales to any window.
fn field_scene(frame: &mut iced::widget::canvas::Frame) {
    use iced::widget::canvas::{gradient, Path, Stroke, Style};
    use iced::Point;
    let (w, h) = (frame.width(), frame.height());
    let rgb = iced::Color::from_rgb8;
    frame.fill_rectangle(
        Point::ORIGIN,
        frame.size(),
        gradient::Linear::new(Point::ORIGIN, Point::new(0.0, h))
            .add_stop(0.0, rgb(0xd6, 0xcb, 0xb0))
            .add_stop(0.38, rgb(0xb9, 0xac, 0x8d))
            .add_stop(0.62, rgb(0x9c, 0x8f, 0x72))
            .add_stop(1.0, rgb(0x7d, 0x70, 0x5a)),
    );
    // A range as (x, y) fractions of the window, closed along the bottom.
    let range = |frame: &mut iced::widget::canvas::Frame, points: &[(f32, f32)], color: iced::Color| {
        let path = Path::new(|b| {
            b.move_to(Point::new(0.0, h));
            for &(x, y) in points {
                b.line_to(Point::new(x * w, y * h));
            }
            b.line_to(Point::new(w, h));
            b.close();
        });
        frame.fill(&path, color);
    };
    range(
        frame,
        &[
            (0.0, 0.46),
            (0.13, 0.38),
            (0.22, 0.42),
            (0.34, 0.28),
            (0.49, 0.40),
            (0.58, 0.35),
            (0.73, 0.43),
            (0.85, 0.33),
            (1.0, 0.42),
        ],
        iced::Color {
            a: 0.55,
            ..rgb(0x8a, 0x7d, 0x63)
        },
    );
    range(
        frame,
        &[
            (0.0, 0.53),
            (0.09, 0.49),
            (0.27, 0.35),
            (0.42, 0.50),
            (0.54, 0.44),
            (0.67, 0.53),
            (0.79, 0.46),
            (1.0, 0.54),
        ],
        iced::Color {
            a: 0.75,
            ..rgb(0x6f, 0x63, 0x4d)
        },
    );
    // The plain.
    let plain = Path::new(|b| {
        b.move_to(Point::new(0.0, 0.65 * h));
        b.bezier_curve_to(
            Point::new(0.2 * w, 0.61 * h),
            Point::new(0.45 * w, 0.67 * h),
            Point::new(0.65 * w, 0.63 * h),
        );
        b.bezier_curve_to(
            Point::new(0.8 * w, 0.61 * h),
            Point::new(0.92 * w, 0.65 * h),
            Point::new(w, 0.64 * h),
        );
        b.line_to(Point::new(w, h));
        b.line_to(Point::new(0.0, h));
        b.close();
    });
    frame.fill(&plain, rgb(0x7b, 0x6e, 0x56));
    // The map grid, every fourth line a sector line.
    let ink = rgb(0x2a, 0x24, 0x18);
    let line = |frame: &mut iced::widget::canvas::Frame, from: Point, to: Point, major: bool| {
        frame.stroke(
            &Path::line(from, to),
            Stroke {
                style: Style::Solid(iced::Color {
                    a: if major { 0.13 } else { 0.06 },
                    ..ink
                }),
                width: 1.0,
                ..Stroke::default()
            },
        );
    };
    const CELL: f32 = 32.0;
    for i in 1..=(w / CELL).ceil() as i32 {
        let x = (i as f32 * CELL).floor() + 0.5;
        line(frame, Point::new(x, 0.0), Point::new(x, h), i % 4 == 0);
    }
    for i in 1..=(h / CELL).ceil() as i32 {
        let y = (i as f32 * CELL).floor() + 0.5;
        line(frame, Point::new(0.0, y), Point::new(w, y), i % 4 == 0);
    }
}

/// The top accent strip, rendered under the HUD bar. 3-px tall,
/// normally a left→right primary→cooler gradient so the rule has
/// motion — not a single flat color stripe across the window.
pub fn hud_scanline_top<'a, M: 'a>() -> Element<'a, M> {
    hud_scanline(crate::ui::theme::is_gay_time().then(|| flag_background(&crate::ui::theme::rainbow_flag_stops())))
}

/// The bottom-edge accent strip.
pub fn hud_scanline_bottom<'a, M: 'a>() -> Element<'a, M> {
    hud_scanline(crate::ui::theme::is_gay_time().then(|| flag_background(&crate::ui::theme::trans_flag_stops())))
}

/// A flat left→right linear gradient through `stops`, packaged as a
/// `Background` ready to drop into a scanline override.
fn flag_background(stops: &[(f32, iced::Color)]) -> iced::Background {
    iced::Background::Gradient(iced::Gradient::Linear(stops.iter().fold(
        iced::gradient::Linear::new(std::f32::consts::FRAC_PI_2),
        |grad, &(offset, color)| grad.add_stop(offset, color),
    )))
}

/// Shared scanline body. `override_bg` replaces the fill when `Some`
/// (e.g. a pride-flag gradient in June); when `None` it falls back to
/// the usual primary→cooler accent rule derived from the live theme.
fn hud_scanline<'a, M: 'a>(override_bg: Option<iced::Background>) -> Element<'a, M> {
    container(
        iced::widget::Space::new()
            .width(Length::Fill)
            .height(Length::Fixed(3.0)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(3.0))
    .style(move |theme: &Theme| {
        let background = override_bg.unwrap_or_else(|| {
            if tango_ui::style::is_advance_wars(theme) {
                return iced::Background::Color(tango_ui::style::aw::RED);
            }
            let primary = theme.palette().primary;
            // Shift the right edge a quarter-turn around the hue
            // wheel (green→teal, blue→violet, red→orange…) so the
            // rule has motion without leaving the accent's family —
            // the old green-tuned channel math landed off-brand
            // colors under other accents.
            let shifted = rotate_hue(primary, 45.0);
            // Re-punch the rotated stop: push it away from gray so
            // the far end burns as hot as the old hand-tuned teal
            // did, instead of a mid-tone accent fading politely.
            let gray = (shifted.r + shifted.g + shifted.b) / 3.0;
            let right = iced::Color {
                r: (gray + (shifted.r - gray) * 1.4).clamp(0.0, 1.0),
                g: (gray + (shifted.g - gray) * 1.4).clamp(0.0, 1.0),
                b: (gray + (shifted.b - gray) * 1.4).clamp(0.0, 1.0),
                a: 1.0,
            };
            iced::Background::Gradient(iced::Gradient::Linear(
                iced::gradient::Linear::new(std::f32::consts::FRAC_PI_2)
                    .add_stop(0.0, primary)
                    .add_stop(1.0, right),
            ))
        });
        iced::widget::container::Style {
            background: Some(background),
            ..Default::default()
        }
    })
    .into()
}

/// HUD frame for inline cards (empty-state hints, lobby side
/// panels, settings groups). The full Legacy Collection treatment:
/// accent-cast plate, glowing accent frame, tech-radius corners —
/// the PET menu's framed panels, not CSS rectangles.
pub fn panel(theme: &Theme) -> iced::widget::container::Style {
    if tango_ui::style::is_advance_wars(theme) {
        return tango_ui::widgets::aw_box();
    }
    let p = theme.extended_palette();
    let bg = theme.palette().background;
    let text = theme.palette().text;
    let primary = theme.palette().primary;
    // Slightly lifted plate. On dark, lift through [`plate_lift`]
    // so the card reads above the body without taking on the
    // accent's hue — the green lives in the frame, not the fill.
    // On light, go toward white so the card looks like paper on
    // parchment.
    let plate = if p.is_dark {
        mix(bg, plate_lift(theme), 0.12)
    } else {
        mix(bg, iced::Color::WHITE, 0.4)
    };
    iced::widget::container::Style {
        background: Some(iced::Background::Color(plate)),
        text_color: Some(text),
        border: iced::Border {
            radius: tech_radius(14.0),
            width: 1.5,
            color: iced::Color {
                a: if p.is_dark { 0.65 } else { 0.45 },
                ..primary
            },
        },
        // On dark the shadow is the frame's accent glow (centered,
        // no offset — light radiating off the border, not a drop
        // shadow). Light theme keeps a soft black drop; a colored
        // glow on a pale page reads as smudge.
        shadow: if p.is_dark {
            iced::Shadow {
                color: iced::Color { a: 0.28, ..primary },
                offset: iced::Vector::new(0.0, 0.0),
                blur_radius: 16.0,
            }
        } else {
            iced::Shadow {
                color: iced::Color {
                    a: 0.18,
                    ..iced::Color::BLACK
                },
                offset: iced::Vector::new(0.0, 6.0),
                blur_radius: 18.0,
            }
        },
        snap: false,
    }
}

/// The scaffolding every modal overlay shares: `panel` (already
/// pop-animated by the caller if it animates) wrapped in a
/// click-swallowing mouse_area and centered, stacked over a dim
/// backdrop wash at `backdrop_alpha` (callers scale their resting
/// alpha by the open-transition's progress so the dim fades with
/// the panel). `dismiss`, when armed, closes the modal on a
/// backdrop click — pass `None` while the modal is animating out
/// so a click mid-fade can't re-fire the close (and for modals
/// that must not be click-dismissed at all).
pub fn modal_layer<'a, M: Clone + 'a>(
    panel: Element<'a, M>,
    backdrop_alpha: f32,
    swallow: M,
    dismiss: Option<M>,
) -> Element<'a, M> {
    let placement = container(sweeten::widget::mouse_area(panel).on_press(move |_| swallow.clone()))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);
    let mut backdrop = sweeten::widget::mouse_area(
        container(iced::widget::Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(crate::ui::anim::backdrop_style(backdrop_alpha)),
    );
    if let Some(m) = dismiss {
        backdrop = backdrop.on_press(move |_| m.clone());
    }
    iced::widget::stack![Element::from(backdrop), Element::from(placement)].into()
}

/// The "you vs opponent" matchup pane shared by the lobby band and
/// the replay detail: the two side cards with a wide gap so the
/// diagonal cut + VS badge from [`vs_splitter`] paints through the
/// middle. The splitter canvas (which also paints the red/blue half
/// tints) is layered *under* the row, so the cards sit on top of
/// the colored plate. Top-aligned so the left card doesn't bounce
/// when the right one grows (the lobby's opponent card gains a line
/// when their settings land).
pub fn matchup_pane<'a, M: 'a>(left: Element<'a, M>, right: Element<'a, M>) -> Element<'a, M> {
    let sides_row = row![left, right].spacing(56).align_y(Alignment::Start);
    container(
        iced::widget::Stack::new()
            .push(
                container(sides_row)
                    .padding(crate::ui::style::PANE_PADDING)
                    .width(Length::Fill),
            )
            .push_under(vs_splitter()),
    )
    .width(Length::Fill)
    .style(pane)
    .into()
}

/// Full-height "VS" splitter: paints a near-vertical band in the
/// body background color through the middle of its bounds so that,
/// when layered behind a padded row of content via
/// `Stack::push_under`, the pane reads as sliced diagonally in
/// half — the body surface peeking through the cut. "VS" sits
/// centered on the band.
///
/// Width and height are both [`Length::Fill`]; the splitter sizes
/// itself to whatever the layered content needs, so the cut
/// reaches the pane's top and bottom edges automatically. See
/// [`matchup_pane`] for the layout pattern.
pub fn vs_splitter<'a, M: 'a>() -> Element<'a, M> {
    use iced::widget::canvas::{Canvas, Frame, LineCap, Path, Stroke, Style};
    use iced::{Point, Rectangle, Renderer};

    /// Thickness of the cut, perpendicular to the band axis. Half
    /// the inter-pane gap so the slice reads as slimmer than the
    /// gaps separating sibling panes — a hairline, not a chasm.
    const BAND_W: f32 = PANE_GAP / 2.0;
    /// Horizontal offset of each band endpoint from the canvas
    /// center. Small relative to typical pane heights so the cut
    /// leans gently rather than racing across the pane — the
    /// "shallow gradient" close-to-vertical look.
    const TILT: f32 = 14.0;
    /// Distance the band extends past the canvas top/bottom edges
    /// before the butt cap kicks in. Has to be > a couple of pixels
    /// or anti-aliasing leaves a soft tapered edge that reads as
    /// the slash trailing off short of the pane border.
    const OVERSHOOT: f32 = 16.0;
    /// "V" / "S" glyph box: per-letter width and cap height of the
    /// hand-drawn letterforms. Roughly what the old 18px font-rendered
    /// glyphs occupied.
    const GLYPH_W: f32 = 10.0;
    const GLYPH_H: f32 = 12.0;
    /// Stroke weight of the letterforms — heavy, keeping the "Black"
    /// weight look of the old font-rendered glyphs.
    const GLYPH_T: f32 = 2.8;
    /// Italic shear: horizontal offset per unit of height above the
    /// glyph's vertical center (≈12°, matching Noto's italic angle).
    const SLANT: f32 = 0.21;
    /// Radius of the body-bg-colored circle that the "VS" sits
    /// inside. Sized so the glyph pair has a comfortable margin
    /// to the rim; the circle merges seamlessly with the band
    /// (same color), reading as a node bulging out of the cut.
    const BADGE_R: f32 = 18.0;
    /// Half the horizontal spread between the V and S glyph
    /// centers. Less than the glyph width so the letter boxes
    /// overlap diagonally — the pair reads as one stamped "VS"
    /// mark — but enough that a hairline channel, parallel to
    /// the cut, stays open between the V's stem and the S's
    /// top bar.
    const GLYPH_DX: f32 = 4.0;
    /// Half the vertical stagger between the V and S glyph
    /// centers. V sits above center, S sits below, giving the
    /// pair a fighting-game-style diagonal stack.
    const GLYPH_DY: f32 = 3.0;

    struct VsDiagonal;

    impl<M> iced::widget::canvas::Program<M> for VsDiagonal {
        type State = ();

        fn draw(
            &self,
            _state: &(),
            renderer: &Renderer,
            theme: &Theme,
            bounds: Rectangle,
            _cursor: iced::mouse::Cursor,
        ) -> Vec<iced::widget::canvas::Geometry> {
            let mut frame = Frame::new(renderer, bounds.size());
            let cx = bounds.width / 2.0;
            let w = bounds.width;
            let h = bounds.height;

            // Player-color tints — left half red (P1), right half
            // blue (P2), split by the diagonal cut. Outer corners
            // are rounded to [`PANE_RADIUS`] so the painted halves
            // match the pane plate's rounded chrome; inner edge is
            // the straight diagonal. Alpha is moderate so the
            // pane plate underneath still reads as the dominant
            // surface and the side cards' text stays legible.
            const PANE_RADIUS: f32 = 4.0;
            let red = iced::Color {
                a: 0.35,
                ..iced::Color::from_rgb(0.85, 0.22, 0.28)
            };
            let blue = iced::Color {
                a: 0.35,
                ..iced::Color::from_rgb(0.18, 0.40, 0.85)
            };
            let left = Path::new(|p| {
                // Start on the top edge, just right of the
                // top-left arc; trace the top edge to the
                // diagonal, down the diagonal, along the bottom
                // edge to the bottom-left arc, then round the two
                // outer corners on the way back up.
                p.move_to(Point::new(PANE_RADIUS, 0.0));
                p.line_to(Point::new(cx + TILT, 0.0));
                p.line_to(Point::new(cx - TILT, h));
                p.line_to(Point::new(PANE_RADIUS, h));
                p.arc_to(Point::new(0.0, h), Point::new(0.0, 0.0), PANE_RADIUS);
                p.arc_to(Point::new(0.0, 0.0), Point::new(w, 0.0), PANE_RADIUS);
                p.close();
            });
            frame.fill(&left, red);
            let right = Path::new(|p| {
                p.move_to(Point::new(w - PANE_RADIUS, 0.0));
                p.line_to(Point::new(cx + TILT, 0.0));
                p.line_to(Point::new(cx - TILT, h));
                p.line_to(Point::new(w - PANE_RADIUS, h));
                p.arc_to(Point::new(w, h), Point::new(w, 0.0), PANE_RADIUS);
                p.arc_to(Point::new(w, 0.0), Point::new(0.0, 0.0), PANE_RADIUS);
                p.close();
            });
            frame.fill(&right, blue);

            // Body-bg-colored band so the pane plate reads as
            // cut, with the page surface showing through. The
            // band has to share the polygons' slope, otherwise
            // the visible diagonals diverge — at the canvas edges
            // the band's centerline would land short of the
            // polygon corner. So extrapolate the polygon line
            // (cx±TILT at y=0/h) out to y=±OVERSHOOT, picking up
            // an extra horizontal swing of `slash_extra` at each
            // end. Butt caps land outside the canvas; visibly the
            // cut meets (and continues past) the pane edges.
            let body_bg = theme.palette().background;
            let slash_extra = TILT * 2.0 * OVERSHOOT / h;
            let line = Path::line(
                Point::new(cx + TILT + slash_extra, -OVERSHOOT),
                Point::new(cx - TILT - slash_extra, h + OVERSHOOT),
            );
            frame.stroke(
                &line,
                Stroke {
                    style: Style::Solid(body_bg),
                    width: BAND_W,
                    line_cap: LineCap::Butt,
                    ..Default::default()
                },
            );

            // Body-bg-colored circle the "VS" sits in. Same color
            // as the band so the two visually fuse into one shape:
            // a slim cut through the pane with a wider node where
            // the badge sits.
            let badge = Path::circle(Point::new(cx, h / 2.0), BADGE_R);
            frame.fill(&badge, body_bg);

            // V upper-left of center, S lower-right of center —
            // the cut runs diagonally between them. The letterforms
            // are hand-traced filled polygons, sheared for the
            // italic lean: none of the bundled Noto faces carry a
            // Black Italic, and two letters aren't worth shipping
            // one for. Heavy and leaned-over so the pair still
            // reads as a fighting-game splash stamped on the slash.
            let cy = h / 2.0;
            let color = muted_color(theme);
            // Outline points are in glyph-local coordinates: origin
            // at the letter's center, y down. The shear leans the
            // top of each letter to the right; horizontal edges stay
            // horizontal, as in a real italic.
            let glyph = |outline: &[(f32, f32)], gx: f32, gy: f32| {
                Path::new(|p| {
                    let mut pts = outline.iter().map(|&(x, y)| Point::new(gx + x - y * SLANT, gy + y));
                    p.move_to(pts.next().unwrap());
                    for pt in pts {
                        p.line_to(pt);
                    }
                    p.close();
                })
            };
            let (gl, gr, gt, gb) = (-GLYPH_W / 2.0, GLYPH_W / 2.0, -GLYPH_H / 2.0, GLYPH_H / 2.0);

            // The V is two thick diagonal strokes meeting in a
            // point. `vt` is the stroke's horizontal cut where it
            // meets the top edge (perpendicular thickness GLYPH_T
            // over the cosine of the stroke's lean); the inner
            // edges run parallel to the outer ones and meet at
            // `apex_y`, leaving a small triangular counter.
            let vt = GLYPH_T * (GLYPH_W / 2.0).hypot(GLYPH_H) / GLYPH_H;
            let v = glyph(
                &[
                    (gl, gt),
                    (gl + vt, gt),
                    (0.0, gt + GLYPH_H * (gr - vt) / gr),
                    (gr - vt, gt),
                    (gr, gt),
                    (0.0, gb),
                ],
                cx - GLYPH_DX,
                cy - GLYPH_DY,
            );
            frame.fill(&v, color);

            // The S is the blocky three-bars-and-two-notches
            // digital form — top aperture opening right, bottom
            // aperture opening left, like the letter. Angular
            // rather than curved both because tracing a curved S
            // by hand is fiddly and because blocky suits the
            // splash style.
            let s = glyph(
                &[
                    (gr, gt),
                    (gr, gt + GLYPH_T),
                    (gl + GLYPH_T, gt + GLYPH_T),
                    (gl + GLYPH_T, -GLYPH_T / 2.0),
                    (gr, -GLYPH_T / 2.0),
                    (gr, gb),
                    (gl, gb),
                    (gl, gb - GLYPH_T),
                    (gr - GLYPH_T, gb - GLYPH_T),
                    (gr - GLYPH_T, GLYPH_T / 2.0),
                    (gl, GLYPH_T / 2.0),
                    (gl, gt),
                ],
                cx + GLYPH_DX,
                cy + GLYPH_DY,
            );
            frame.fill(&s, color);

            vec![frame.into_geometry()]
        }
    }

    Canvas::new(VsDiagonal).width(Length::Fill).height(Length::Fill).into()
}

/// The battlefield seat colors: this side reads red, the opponent blue —
/// one pair everywhere a you-vs-opponent split is drawn (the PvP
/// telemetry header's seat dots, the HP graphs and their legends).
pub const FIELD_RED: iced::Color = iced::Color::from_rgb(0.85, 0.22, 0.28);
pub const FIELD_BLUE: iced::Color = iced::Color::from_rgb(0.18, 0.40, 0.85);
