//! Putting the emulator's frames on screen: the main pane and the
//! opponent's pane or picture-in-picture inset.

use super::priming::{priming_copy, priming_notice};
use super::*;
// Explicit so these win over iced's prelude macros; see the parent module.
use sweeten::widget::{column, row};

/// Native size of the active session's frame. The session knows its
/// console's screen from boot, so the pane holds the right shape before
/// the first frame lands.
fn native_size(ctx: Ctx<'_>) -> (u32, u32) {
    ctx.state
        .active
        .as_ref()
        .map(|s| s.frame_size())
        // Unreachable in practice — the app only renders this view over
        // an active session — and an absent one draws only the black
        // placeholder, where shape doesn't matter.
        .unwrap_or((1, 1))
}

/// The frame's on-screen size in a pane of `size`: an exact integer
/// multiple of `img` (crisp, the default) or, with fractional scaling, a
/// smooth aspect-fit.
fn fit(size: iced::Size, (img_w, img_h): (f32, f32), fractional_scaling: bool) -> (f32, f32) {
    let raw = (size.width / img_w).min(size.height / img_h);
    let scale = if fractional_scaling {
        raw.max(0.0)
    } else {
        raw.floor().max(1.0)
    };
    (img_w * scale, img_h * scale)
}

/// Seat a `w` × `h` frame in its pane at the given alignment. An
/// integer-scaled frame gets a tight container so its drop shadow traces
/// the frame's edges, not the surrounding pane; a smooth aspect-fit gets
/// none.
fn place<'a>(
    fb: Element<'a, Message>,
    (w, h): (f32, f32),
    fractional_scaling: bool,
    horizontal_alignment: iced::alignment::Horizontal,
    vertical_alignment: iced::alignment::Vertical,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = if fractional_scaling {
        fb
    } else {
        container(fb)
            .width(Length::Fixed(w))
            .height(Length::Fixed(h))
            .style(|_theme: &iced::Theme| iced::widget::container::Style {
                shadow: iced::Shadow {
                    color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.55),
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 24.0,
                },
                ..Default::default()
            })
            .into()
    };
    container(content)
        .width(Fill)
        .height(Fill)
        .align_x(horizontal_alignment)
        .align_y(vertical_alignment)
        .into()
}

/// The live framebuffer, rendered through a custom wgpu shader widget
/// (one persistent GPU texture, written in place each vblank) instead
/// of a per-frame `image` handle. The shader fills the widget's
/// bounds, so the widget is sized to the framebuffer rect — an exact
/// integer multiple (crisp, the default) or a smooth aspect-fit —
/// using `responsive` for the pane size both need. Before the first
/// frame, a 1×1 black placeholder keeps the pane opaque.
pub(super) fn framebuffer_view<'a>(
    ctx: Ctx<'a>,
    // Stacked layouts dock the two frames against their center seam; every
    // other presentation centers the main frame in its pane.
    horizontal_alignment: iced::alignment::Horizontal,
    vertical_alignment: iced::alignment::Vertical,
) -> Element<'a, Message> {
    let state = ctx.state;
    let (fractional_scaling, effect) = (ctx.fractional_scaling, ctx.effect);
    // Resolved out here, where the language is still in hand; the
    // closure below only lays it out (see `priming_notice`).
    let priming = priming_copy(ctx.lang, state);
    // The widget is sized to native·scale — the same rectangle the old CPU
    // upscalers produced — and the effect's fragment shader magnifies the
    // native texture to fill it.
    let (native_w, native_h) = native_size(ctx);
    let img = ((native_w * effect.scale) as f32, (native_h * effect.scale) as f32);
    let base_frame = state
        .current_frame
        .clone()
        .unwrap_or_else(crate::platform::video::framebuffer::Frame::black);

    iced::widget::responsive(move |size| {
        let (w, h) = fit(size, img, fractional_scaling);
        let mut frame = base_frame.clone();
        // The uploaded texture is always the native frame; the effect is just
        // the draw-time pipeline pick. Take it live from config here (not from
        // whatever was current when the frame was produced) so switching the
        // video filter re-renders immediately — even on a paused replay that
        // isn't producing new frames.
        frame.effect = effect;
        let fb = iced::widget::shader::Shader::new(crate::platform::video::framebuffer::Program::new(frame))
            .width(Length::Fixed(w))
            .height(Length::Fixed(h));
        let mut fb: Element<'a, Message> = fb.into();
        // The priming notice, bounded to the frame rect itself so it
        // reads as part of the screen rather than of the window. Last
        // in, so its own dismissal (the failure case) is the thing a
        // press lands on.
        if let Some(copy) = priming.as_ref() {
            fb = stack![fb, priming_notice(copy, w, h)].into();
        }

        place(fb, (w, h), fractional_scaling, horizontal_alignment, vertical_alignment)
    })
    .into()
}

/// The auxiliary opponent framebuffer as a full pane. This uses the PiP
/// primitive type so iced gives it an independent resident GPU texture, but
/// otherwise mirrors the main framebuffer's scaling, filter, and shadow
/// treatment.
fn opponent_framebuffer_view<'a>(
    ctx: Ctx<'a>,
    horizontal_alignment: iced::alignment::Horizontal,
    vertical_alignment: iced::alignment::Vertical,
) -> Element<'a, Message> {
    let (fractional_scaling, effect) = (ctx.fractional_scaling, ctx.effect);
    let (native_w, native_h) = native_size(ctx);
    let img = ((native_w * effect.scale) as f32, (native_h * effect.scale) as f32);
    // Keep the equal pane mounted while its first frame is being captured.
    // The black pixel stretches into the same aspect-sized widget, avoiding a
    // one-frame layout jump when the session publishes the auxiliary surface.
    let base_frame = ctx
        .state
        .pip_frame
        .clone()
        .unwrap_or_else(crate::platform::video::framebuffer::Frame::black);

    iced::widget::responsive(move |size| {
        let (w, h) = fit(size, img, fractional_scaling);
        let mut frame = base_frame.clone();
        frame.effect = effect;
        let fb = iced::widget::shader::Shader::new(crate::platform::video::framebuffer::PipProgram::new(frame))
            .width(Length::Fixed(w))
            .height(Length::Fixed(h));
        place(
            fb.into(),
            (w, h),
            fractional_scaling,
            horizontal_alignment,
            vertical_alignment,
        )
    })
    .into()
}

/// Split the emulator body into two equal perspective panes along the
/// selected axis. Both arrangements have a zero-width center seam.
pub(super) fn stacked_framebuffers<'a>(
    ctx: Ctx<'a>,
    main: Element<'a, Message>,
    view: crate::config::OpponentView,
) -> Element<'a, Message> {
    match view {
        crate::config::OpponentView::StackHorizontally => {
            let opponent = opponent_framebuffer_view(
                ctx,
                iced::alignment::Horizontal::Left,
                iced::alignment::Vertical::Center,
            );
            row![
                container(main).width(Length::FillPortion(1)).height(Fill),
                container(opponent).width(Length::FillPortion(1)).height(Fill),
            ]
            .spacing(0)
            .padding([0, 12])
            .width(Fill)
            .height(Fill)
            .into()
        }
        crate::config::OpponentView::StackVertically => {
            let opponent =
                opponent_framebuffer_view(ctx, iced::alignment::Horizontal::Center, iced::alignment::Vertical::Top);
            column![
                container(main).width(Fill).height(Length::FillPortion(1)),
                container(opponent).width(Fill).height(Length::FillPortion(1)),
            ]
            .spacing(0)
            .padding([12, 0])
            .width(Fill)
            .height(Fill)
            .into()
        }
        _ => main,
    }
}

/// Picture-in-picture inset, top-right below the corner commands: the
/// other side's screen. Both cores render anyway (replay re-simulates the
/// opponent, training runs a live pair); this just insets the extra one.
/// Drawn through its own shader surface ([`PipProgram`]) because the main
/// framebuffer's pipeline owns a single resident texture. Reads the
/// host's captured [`State::pip_frame`], so it's pure presentation with no
/// message of its own — every session kind can push it directly.
///
/// [`PipProgram`]: crate::platform::video::framebuffer::PipProgram
pub(crate) fn pip_overlay<'a>(ctx: Ctx<'a>) -> Option<Element<'a, Message>> {
    let frame = ctx.state.pip_frame.clone()?;
    // 1.5x native: readable without dominating the main view.
    let (w, h) = (frame.width as f32 * 1.5, frame.height as f32 * 1.5);
    let fb = iced::widget::shader::Shader::new(crate::platform::video::framebuffer::PipProgram::new(frame))
        .width(Length::Fixed(w))
        .height(Length::Fixed(h));
    let plate = container(fb).padding(3).style(hud_chip_plate);
    Some(
        container(plate)
            .width(Fill)
            .height(Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Top)
            .padding(iced::Padding {
                // Clear the corner commands' resting spot.
                top: 56.0,
                right: 12.0,
                bottom: 0.0,
                left: 0.0,
            })
            .into(),
    )
}
