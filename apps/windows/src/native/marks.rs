//! Small drawn things: the playing bars (as on the website's `.eq`) and the Volvix logo.
//! They draw themselves, so the bars animate without rebuilding the screen.
use std::sync::OnceLock;
use std::time::Instant;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::widget::{tree::Tree, Widget};
use iced::advanced::{mouse, Clipboard, Shell};
use iced::event::{Event, Status};
use iced::window::{self, RedrawRequest};
use iced::{Border, Color, Element, Length, Rectangle, Shadow, Size};

use crate::ui;

fn start() -> Instant {
    static T: OnceLock<Instant> = OnceLock::new();
    *T.get_or_init(Instant::now)
}

/// Three bars bouncing while a song plays; still (at different heights) while paused.
pub struct EqBars {
    playing: bool,
    w: f32,
    h: f32,
    color: Color,
}

pub fn eq<'a, M: 'a>(playing: bool, h: f32) -> Element<'a, M> {
    Element::new(EqBars { playing, w: h * 14.0 / 12.0, h, color: ui::ACCENT })
}

/// The website's `@keyframes eq`: 25% → 100% height, 0.8 s, ease-in-out, alternating.
fn bar_height(t: f32, delay: f32) -> f32 {
    let p = ((t - delay) / 0.8).rem_euclid(2.0);
    let x = if p < 1.0 { p } else { 2.0 - p };
    let eased = 0.5 - 0.5 * (std::f32::consts::PI * x).cos();
    0.25 + 0.75 * eased
}

impl<M, Th, R: renderer::Renderer> Widget<M, Th, R> for EqBars {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.w), Length::Fixed(self.h))
    }

    fn layout(&self, _tree: &mut Tree, _r: &R, _limits: &layout::Limits) -> layout::Node {
        layout::Node::new(Size::new(self.w, self.h))
    }

    fn draw(&self, _tree: &Tree, renderer: &mut R, _theme: &Th, _style: &renderer::Style, layout: Layout<'_>, _cursor: mouse::Cursor, _viewport: &Rectangle) {
        let b = layout.bounds();
        let t = start().elapsed().as_secs_f32();
        let bar = (self.w - 4.0) / 3.0;
        for (i, delay) in [0.0f32, 0.3, 0.55].iter().enumerate() {
            let f = if self.playing { bar_height(t, -delay) } else { [0.45, 0.85, 0.6][i] };
            let h = (b.height * f).max(1.0);
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle { x: b.x + i as f32 * (bar + 2.0), y: b.y + b.height - h, width: bar, height: h },
                    border: Border { radius: 1.0.into(), width: 0.0, color: Color::TRANSPARENT },
                    shadow: Shadow::default(),
                },
                self.color,
            );
        }
    }

    fn on_event(&mut self, _tree: &mut Tree, event: Event, _layout: Layout<'_>, _cursor: mouse::Cursor, _r: &R, _c: &mut dyn Clipboard, shell: &mut Shell<'_, M>, _viewport: &Rectangle) -> Status {
        if self.playing {
            if let Event::Window(window::Event::RedrawRequested(_)) = event {
                shell.request_redraw(RedrawRequest::NextFrame);
            }
        }
        Status::Ignored
    }
}

/// The Volvix logo (web/index.html): a red disc, a faint ring, a black centre, on a 32-unit grid.
pub struct Logo {
    size: f32,
}

pub fn logo<'a, M: 'a>(size: f32) -> Element<'a, M> {
    Element::new(Logo { size })
}

impl<M, Th, R: renderer::Renderer> Widget<M, Th, R> for Logo {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.size), Length::Fixed(self.size))
    }

    fn layout(&self, _tree: &mut Tree, _r: &R, _limits: &layout::Limits) -> layout::Node {
        layout::Node::new(Size::new(self.size, self.size))
    }

    fn draw(&self, _tree: &Tree, renderer: &mut R, _theme: &Th, _style: &renderer::Style, layout: Layout<'_>, _cursor: mouse::Cursor, _viewport: &Rectangle) {
        let b = layout.bounds();
        let u = self.size / 32.0;
        let (cx, cy) = (b.x + 16.0 * u, b.y + 16.0 * u);
        let circle = |r: f32| Rectangle { x: cx - r, y: cy - r, width: 2.0 * r, height: 2.0 * r };
        let none = Color::TRANSPARENT;
        // disc r=15
        renderer.fill_quad(Quad { bounds: circle(15.0 * u), border: Border { radius: (15.0 * u).into(), width: 0.0, color: none }, shadow: Shadow::default() }, ui::ACCENT);
        // ring r=9.5, 1-unit stroke, black at 22%
        renderer.fill_quad(
            Quad { bounds: circle(10.0 * u), border: Border { radius: (10.0 * u).into(), width: u.max(1.0), color: Color { a: 0.22, ..Color::BLACK } }, shadow: Shadow::default() },
            none,
        );
        // centre r=3.5, #111
        renderer.fill_quad(Quad { bounds: circle(3.5 * u), border: Border { radius: (3.5 * u).into(), width: 0.0, color: none }, shadow: Shadow::default() }, Color::from_rgb8(0x11, 0x11, 0x11));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_move_between_quarter_and_full() {
        assert!((bar_height(0.0, 0.0) - 0.25).abs() < 1e-4);
        assert!((bar_height(0.8, 0.0) - 1.0).abs() < 1e-4);
        assert!((bar_height(1.6, 0.0) - 0.25).abs() < 1e-4);
        assert!(bar_height(0.4, 0.0) > 0.5 && bar_height(0.4, 0.0) < 0.75);
    }
}
