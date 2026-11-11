//! Small drawn things: the playing bars (as on the website's `.eq`) and the Volvix logo.
//! They draw themselves, so the bars animate without rebuilding the screen.
use std::cell::Cell;
use std::sync::OnceLock;
use std::time::Instant;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::widget::{tree::{self, Tree}, Widget};
use iced::advanced::{mouse, Clipboard, Shell};
use iced::event::{Event, Status};
use iced::window::{self, RedrawRequest};
use iced::{Border, Color, Element, Length, Rectangle, Shadow, Size};

use crate::ui;

fn start() -> Instant {
    static T: OnceLock<Instant> = OnceLock::new();
    *T.get_or_init(Instant::now)
}

/// Seconds the bars take to sink after a pause.
const SETTLE: f32 = 0.5;

/// What the bars remember between frames: where they were, and when they were paused.
struct Settle {
    last: Cell<[f32; 3]>,
    paused_at: Cell<Option<f32>>,
}

/// Three bars bouncing while a song plays; they sink to the bottom when it's paused.
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

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Settle>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Settle { last: Cell::new([0.25; 3]), paused_at: Cell::new(Some(-10.0)) })
    }

    fn layout(&self, _tree: &mut Tree, _r: &R, _limits: &layout::Limits) -> layout::Node {
        layout::Node::new(Size::new(self.w, self.h))
    }

    fn draw(&self, tree: &Tree, renderer: &mut R, _theme: &Th, _style: &renderer::Style, layout: Layout<'_>, _cursor: mouse::Cursor, _viewport: &Rectangle) {
        let b = layout.bounds();
        let t = start().elapsed().as_secs_f32();
        let bar = (self.w - 4.0) / 3.0;
        let st = tree.state.downcast_ref::<Settle>();
        if self.playing {
            st.paused_at.set(None);
        } else if st.paused_at.get().is_none() {
            st.paused_at.set(Some(t)); // just paused: the bars sink from where they were
        }
        for (i, delay) in [0.0f32, 0.3, 0.55].iter().enumerate() {
            let f = if self.playing {
                let f = bar_height(t, -delay);
                let mut l = st.last.get();
                l[i] = f;
                st.last.set(l);
                f
            } else {
                // paused: ease down to the lowest level, like the bars dropping when the music stops
                let k = ((t - st.paused_at.get().unwrap_or(t)) / SETTLE).clamp(0.0, 1.0);
                let from = st.last.get()[i];
                from + (0.25 - from) * (1.0 - (1.0 - k).powi(3))
            };
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

    fn on_event(&mut self, tree: &mut Tree, event: Event, _layout: Layout<'_>, _cursor: mouse::Cursor, _r: &R, _c: &mut dyn Clipboard, shell: &mut Shell<'_, M>, _viewport: &Rectangle) -> Status {
        // keep drawing while playing, and for the moment the bars take to sink after a pause
        let now = start().elapsed().as_secs_f32();
        let st = tree.state.downcast_ref::<Settle>();
        // note the moment of pausing here (not only when drawing), so the sinking starts at once and keeps getting frames
        if self.playing {
            st.paused_at.set(None);
        } else if st.paused_at.get().is_none() {
            st.paused_at.set(Some(now));
        }
        let settling = st.paused_at.get().map(|p| now - p < SETTLE).unwrap_or(false);
        if self.playing || settling {
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
    /// drawn this much smaller or larger around its centre (the launch screen breathes)
    scale: f32,
}

pub fn logo<'a, M: 'a>(size: f32) -> Element<'a, M> {
    Element::new(Logo { size, scale: 1.0 })
}

pub fn logo_scaled<'a, M: 'a>(size: f32, scale: f32) -> Element<'a, M> {
    Element::new(Logo { size, scale })
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
        let (cx, cy) = (b.x + self.size / 2.0, b.y + self.size / 2.0);
        let u = self.size * self.scale / 32.0;
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

/// A slider that follows the mouse by itself while dragged: the handle is drawn from the drag
/// position straight away, and the app only hears about it when you let go (seek) or every
/// `every` ms (volume). So dragging never waits for the whole window to be rebuilt.
pub struct Scrub<'a, M> {
    value: f32,
    max: f32,
    on_change: Option<Box<dyn Fn(f32) -> M + 'a>>,
    on_release: Box<dyn Fn(f32) -> M + 'a>,
    every: std::time::Duration,
}

#[derive(Default)]
struct ScrubState {
    dragging: bool,
    value: f32,
    last_sent: Option<Instant>,
}

pub fn scrub<'a, M: 'a>(value: f32, max: f32, on_release: impl Fn(f32) -> M + 'a) -> Scrub<'a, M> {
    Scrub { value, max: max.max(0.0001), on_change: None, on_release: Box::new(on_release), every: std::time::Duration::from_millis(50) }
}

impl<'a, M> Scrub<'a, M> {
    /// Also report while dragging (at most every 50 ms).
    pub fn on_change(mut self, f: impl Fn(f32) -> M + 'a) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    fn at(&self, bounds: Rectangle, x: f32) -> f32 {
        ((x - bounds.x) / bounds.width).clamp(0.0, 1.0) * self.max
    }
}

const SCRUB_H: f32 = 16.0;

impl<'a, M: Clone, Th, R: renderer::Renderer> Widget<M, Th, R> for Scrub<'a, M> {
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<ScrubState>()
    }
    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(ScrubState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fixed(SCRUB_H))
    }

    fn layout(&self, _tree: &mut Tree, _r: &R, limits: &layout::Limits) -> layout::Node {
        layout::Node::new(limits.resolve(Length::Fill, Length::Fixed(SCRUB_H), Size::ZERO))
    }

    fn draw(&self, tree: &Tree, renderer: &mut R, _theme: &Th, _style: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, _viewport: &Rectangle) {
        let st = tree.state.downcast_ref::<ScrubState>();
        let b = layout.bounds();
        let v = if st.dragging { st.value } else { self.value };
        let f = (v / self.max).clamp(0.0, 1.0);
        let live = st.dragging || cursor.is_over(b);
        let rail = Rectangle { x: b.x, y: b.center_y() - 2.0, width: b.width, height: 4.0 };
        let round = Border { radius: 2.0.into(), width: 0.0, color: Color::TRANSPARENT };
        renderer.fill_quad(Quad { bounds: rail, border: round, shadow: Shadow::default() }, Color { a: 0.2, ..ui::TEXT });
        renderer.fill_quad(Quad { bounds: Rectangle { width: rail.width * f, ..rail }, border: round, shadow: Shadow::default() }, if live { ui::ACCENT } else { ui::TEXT });
        if live {
            let r = 6.0;
            renderer.fill_quad(
                Quad {
                    bounds: Rectangle { x: b.x + b.width * f - r, y: b.center_y() - r, width: 2.0 * r, height: 2.0 * r },
                    border: Border { radius: r.into(), width: 0.0, color: Color::TRANSPARENT },
                    shadow: Shadow::default(),
                },
                ui::TEXT,
            );
        }
    }

    fn on_event(&mut self, tree: &mut Tree, event: Event, layout: Layout<'_>, cursor: mouse::Cursor, _r: &R, _c: &mut dyn Clipboard, shell: &mut Shell<'_, M>, _viewport: &Rectangle) -> Status {
        let st = tree.state.downcast_mut::<ScrubState>();
        let b = layout.bounds();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(p) = cursor.position_over(b) {
                    st.dragging = true;
                    st.value = self.at(b, p.x);
                    st.last_sent = Some(Instant::now());
                    if let Some(f) = &self.on_change {
                        shell.publish(f(st.value));
                    }
                    shell.request_redraw(RedrawRequest::NextFrame);
                    return Status::Captured;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if st.dragging => {
                st.value = self.at(b, position.x);
                if let Some(f) = &self.on_change {
                    if st.last_sent.map(|t| t.elapsed() >= self.every).unwrap_or(true) {
                        st.last_sent = Some(Instant::now());
                        shell.publish(f(st.value));
                    }
                }
                shell.request_redraw(RedrawRequest::NextFrame);
                return Status::Captured;
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if st.dragging => {
                st.dragging = false;
                shell.publish((self.on_release)(st.value));
                return Status::Captured;
            }
            _ => {}
        }
        Status::Ignored
    }

    fn mouse_interaction(&self, _tree: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, _viewport: &Rectangle, _r: &R) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }
}

impl<'a, M: Clone + 'a, Th: 'a, R: renderer::Renderer + 'a> From<Scrub<'a, M>> for Element<'a, M, Th, R> {
    fn from(s: Scrub<'a, M>) -> Self {
        Element::new(s)
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
