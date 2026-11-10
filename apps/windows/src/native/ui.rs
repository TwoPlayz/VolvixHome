//! Look and feel: the website's dark palette, Windows' own fonts and icons, and small widget helpers.
use iced::widget::{button, container, image, text, Space};
use iced::{border, Background, Border, Color, Element, Font, Length, Shadow, Theme, Vector};

pub const ACCENT: Color = rgb(0xE5, 0x48, 0x4D);
pub const ACCENT_HOVER: Color = rgb(0xD6, 0x3D, 0x42);
pub const FRAME: Color = rgb(0x00, 0x00, 0x00);
pub const SURFACE: Color = rgb(0x12, 0x12, 0x14);
pub const SURFACE_2: Color = rgb(0x19, 0x19, 0x1C);
pub const SURFACE_3: Color = rgb(0x24, 0x24, 0x28);
pub const INPUT_BG: Color = rgb(0x1C, 0x1C, 0x1F);
pub const TEXT: Color = rgb(0xF5, 0xF5, 0xF7);
pub const TEXT_2: Color = rgb(0xA1, 0xA1, 0xA8);
pub const TEXT_3: Color = rgb(0x86, 0x86, 0x8E);
pub const HOVER: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.06 };
pub const ACTIVE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.10 };
pub const BORDER: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 };

pub const RADIUS: f32 = 10.0;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
}

/// Windows 11's UI font (falls back to the default if missing).
pub const FONT: Font = Font::with_name("Segoe UI Variable Text");
pub const FONT_BOLD: Font = Font { weight: iced::font::Weight::Semibold, ..Font::with_name("Segoe UI Variable Display") };
/// Windows' own icon font (Windows 11; Windows 10 has the same glyphs in Segoe MDL2 Assets).
pub const ICONS: Font = Font::with_name("Segoe Fluent Icons");

pub mod icon {
    pub const PLAY: char = '\u{E768}';
    pub const PAUSE: char = '\u{E769}';
    pub const PREVIOUS: char = '\u{E892}';
    pub const NEXT: char = '\u{E893}';
    pub const SHUFFLE: char = '\u{E8B1}';
    pub const REPEAT_ALL: char = '\u{E8EE}';
    pub const REPEAT_ONE: char = '\u{E8ED}';
    pub const VOLUME: char = '\u{E767}';
    pub const MUTE: char = '\u{E74F}';
    pub const HOME: char = '\u{E80F}';
    pub const SEARCH: char = '\u{E721}';
    pub const LIBRARY: char = '\u{E8F1}';
    pub const QUEUE: char = '\u{E90B}';
    pub const ADD: char = '\u{E710}';
    pub const HEART: char = '\u{EB51}';
    pub const HEART_FILL: char = '\u{EB52}';
    pub const DOWNLOAD: char = '\u{E896}';
    pub const DOWNLOADED: char = '\u{E73E}';
    pub const DELETE: char = '\u{E74D}';
    pub const UP: char = '\u{E70E}';
    pub const DOWN: char = '\u{E70D}';
    pub const EDIT: char = '\u{E70F}';
    pub const BELL: char = '\u{EA8F}';
    pub const PEOPLE: char = '\u{E716}';
    pub const SETTINGS: char = '\u{E713}';
    pub const LYRICS: char = '\u{E8BD}';
    pub const GLOBE: char = '\u{E774}';
    pub const PLAYLIST: char = '\u{E8FD}';
    pub const CLOSE: char = '\u{E711}';
    pub const PLAY_NEXT: char = '\u{E893}';
}

pub fn icon<'a>(c: char, size: u16) -> iced::widget::Text<'a, Theme> {
    text(c.to_string()).font(ICONS).size(size as f32).line_height(1.0)
}

pub fn theme() -> Theme {
    Theme::custom(
        "Volvix Home".into(),
        iced::theme::Palette { background: FRAME, text: TEXT, primary: ACCENT, success: rgb(0x30, 0xA4, 0x6C), danger: ACCENT },
    )
}

pub fn title<'a, M: 'a>(s: impl text::IntoFragment<'a>, size: u16) -> Element<'a, M> {
    text(s).font(FONT_BOLD).size(size as f32).color(TEXT).into()
}
pub fn dim<'a>(s: impl text::IntoFragment<'a>, size: u16) -> iced::widget::Text<'a, Theme> {
    text(s).size(size as f32).color(TEXT_2)
}

/* ---------- containers ---------- */

pub fn panel(_: &Theme) -> container::Style {
    container::Style { background: Some(SURFACE.into()), border: border::rounded(RADIUS), ..Default::default() }
}
pub fn frame(_: &Theme) -> container::Style {
    container::Style { background: Some(FRAME.into()), text_color: Some(TEXT), ..Default::default() }
}
pub fn error_box(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color { a: 0.14, ..ACCENT }.into()),
        border: Border { radius: 8.0.into(), width: 1.0, color: Color { a: 0.4, ..ACCENT } },
        text_color: Some(TEXT),
        ..Default::default()
    }
}

/* ---------- buttons ---------- */

/// A row or tile that only shows it's clickable on hover.
pub fn flat(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(HOVER.into()),
        button::Status::Pressed => Some(ACTIVE.into()),
        _ => None,
    };
    button::Style { background: bg, text_color: TEXT, border: border::rounded(8.0), ..Default::default() }
}
/// A sidebar item, highlighted when it's the current page.
pub fn nav(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t, s| {
        let mut st = flat(t, s);
        if active {
            st.background = Some(ACTIVE.into());
        }
        st.text_color = if active { TEXT } else { TEXT_2 };
        if matches!(s, button::Status::Hovered) {
            st.text_color = TEXT;
        }
        st
    }
}
/// Icon buttons in the player; `on` = shuffle/repeat switched on (red).
pub fn icon_btn(on: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| button::Style {
        background: None,
        text_color: match (on, s) {
            (true, _) => ACCENT,
            (false, button::Status::Hovered) => TEXT,
            (false, button::Status::Disabled) => TEXT_3,
            _ => TEXT_2,
        },
        ..Default::default()
    }
}
/// The big round play button.
pub fn round_play(_: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(if matches!(s, button::Status::Hovered) { rgb(0xFF, 0xFF, 0xFF) } else { TEXT })),
        text_color: FRAME,
        border: border::rounded(100.0),
        ..Default::default()
    }
}
pub fn primary(_: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: Some(match s {
            button::Status::Hovered | button::Status::Pressed => ACCENT_HOVER,
            button::Status::Disabled => Color { a: 0.5, ..ACCENT },
            _ => ACCENT,
        }
        .into()),
        text_color: rgb(0xFF, 0xFF, 0xFF),
        border: border::rounded(100.0),
        shadow: Shadow { color: Color::TRANSPARENT, offset: Vector::ZERO, blur_radius: 0.0 },
    }
}
pub fn secondary(_: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: Some(if matches!(s, button::Status::Hovered) { SURFACE_3 } else { SURFACE_2 }.into()),
        text_color: TEXT,
        border: Border { radius: 100.0.into(), width: 1.0, color: BORDER },
        ..Default::default()
    }
}
/// Library tabs ("Albums", "Artists").
pub fn chip(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| button::Style {
        background: Some(if active { TEXT } else if matches!(s, button::Status::Hovered) { SURFACE_3 } else { SURFACE_2 }.into()),
        text_color: if active { FRAME } else { TEXT },
        border: border::rounded(100.0),
        ..Default::default()
    }
}

pub fn input(_: &Theme, s: iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    use iced::widget::text_input::{Status, Style};
    Style {
        background: INPUT_BG.into(),
        border: Border { radius: 8.0.into(), width: 1.0, color: if matches!(s, Status::Focused) { TEXT_2 } else { BORDER } },
        icon: TEXT_2,
        placeholder: TEXT_3,
        value: TEXT,
        selection: Color { a: 0.35, ..ACCENT },
    }
}

pub fn slider(_: &Theme, s: iced::widget::slider::Status) -> iced::widget::slider::Style {
    use iced::widget::slider::{Handle, HandleShape, Rail, Style};
    let hovered = !matches!(s, iced::widget::slider::Status::Active);
    Style {
        rail: Rail {
            backgrounds: ((if hovered { ACCENT } else { TEXT }).into(), Color { a: 0.2, ..TEXT }.into()),
            width: 4.0,
            border: border::rounded(2.0),
        },
        handle: Handle {
            // hidden until hovered (never zero-sized: the software renderer refuses empty shapes)
            shape: HandleShape::Circle { radius: 6.0 },
            background: if hovered { TEXT } else { Color::TRANSPARENT }.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

pub fn scroller(_: &Theme, s: iced::widget::scrollable::Status) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{Rail, Scroller, Status, Style};
    let shown = !matches!(s, Status::Active);
    let rail = Rail {
        background: None,
        border: border::rounded(4.0),
        scroller: Scroller { color: if shown { Color { a: 0.25, ..TEXT } } else { Color::TRANSPARENT }, border: border::rounded(4.0) },
    };
    Style { container: container::Style::default(), vertical_rail: rail, horizontal_rail: rail, gap: None }
}

/* ---------- covers ---------- */

/// An album cover at `size` px: the real picture, or a drawn stand-in (no art, or still loading).
pub fn cover<'a, M: 'a>(handle: Option<&image::Handle>, title: &str, size: f32) -> Element<'a, M> {
    match handle {
        Some(h) => image(h.clone()).width(size).height(size).content_fit(iced::ContentFit::Cover).into(),
        None => {
            let tint = tint_for(title);
            let mark = match title.chars().find(|c| c.is_alphanumeric()) {
                Some(c) => text(c.to_uppercase().collect::<String>()).font(FONT_BOLD),
                None => text('\u{E8D6}'.to_string()).font(ICONS), // a note, when there's no title
            };
            container(mark.size(size * 0.38).color(Color { a: 0.85, ..TEXT }))
                .width(size)
                .height(size)
                .center_x(size)
                .center_y(size)
                .style(move |_| container::Style { background: Some(tint.into()), border: border::rounded(6.0), ..Default::default() })
                .into()
        }
    }
}

/// A steady colour per album, so drawn covers are told apart.
fn tint_for(s: &str) -> Color {
    let h = s.bytes().fold(5381u32, |h, b| h.wrapping_mul(33) ^ b as u32);
    let palette = [rgb(0x3A, 0x2A, 0x4F), rgb(0x1F, 0x3B, 0x4D), rgb(0x4A, 0x2C, 0x2C), rgb(0x24, 0x40, 0x34), rgb(0x45, 0x3A, 0x22), rgb(0x2B, 0x2F, 0x4A)];
    palette[(h as usize) % palette.len()]
}

/// The see-through layer behind a dialog.
pub fn scrim(_: &Theme) -> container::Style {
    container::Style { background: Some(Color { a: 0.6, ..FRAME }.into()), ..Default::default() }
}
/// A dialog's card.
pub fn dialog(_: &Theme) -> container::Style {
    container::Style {
        background: Some(SURFACE_2.into()),
        border: Border { radius: 14.0.into(), width: 1.0, color: BORDER },
        text_color: Some(TEXT),
        shadow: Shadow { color: Color { a: 0.5, ..FRAME }, offset: Vector::new(0.0, 12.0), blur_radius: 32.0 },
    }
}
/// The little message that pops up above the player.
pub fn toast(_: &Theme) -> container::Style {
    container::Style { background: Some(TEXT.into()), text_color: Some(FRAME), border: border::rounded(8.0), ..Default::default() }
}
/// The unread count on the bell.
pub fn badge(_: &Theme) -> container::Style {
    container::Style { background: Some(ACCENT.into()), text_color: Some(rgb(0xFF, 0xFF, 0xFF)), border: border::rounded(9.0), ..Default::default() }
}
/// Mix cards: a coloured tile per mix.
pub fn mix_tile(seed: &str) -> impl Fn(&Theme) -> container::Style {
    let tint = tint_for(seed);
    move |_| container::Style {
        background: Some(Background::Gradient(iced::Gradient::Linear(iced::gradient::Linear::new(0.8).add_stop(0.0, Color { a: 1.0, ..ACCENT }).add_stop(1.0, tint)))),
        border: border::rounded(6.0),
        text_color: Some(TEXT),
        ..Default::default()
    }
}

pub fn gap<'a, M: 'a>(px: u16) -> Element<'a, M> {
    Space::with_height(Length::Fixed(px as f32)).into()
}
