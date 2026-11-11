//! Look and feel: the website's dark palette, Windows' own fonts and icons, and small widget helpers.
use iced::widget::{button, container, image, text, Space};
use iced::{border, Background, Border, Color, Element, Font, Length, Shadow, Theme, Vector};

/// Light or dark: the whole app's colours follow this (set from Settings → Appearance, or the system).
static LIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_light(on: bool) {
    LIGHT.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub fn is_light() -> bool {
    LIGHT.load(std::sync::atomic::Ordering::Relaxed)
}

/// A colour with a dark and a light version, the website's (web/css/styles.css). Hover, active and border are the
/// website's see-through white/black as solid colours (a see-through colour is drawn too bright here).
macro_rules! themed {
    ($name:ident, $dark:expr, $light:expr) => {
        #[allow(non_snake_case)]
        pub fn $name() -> Color {
            if is_light() {
                $light
            } else {
                $dark
            }
        }
    };
}
themed!(ACCENT, rgb(0xE5, 0x48, 0x4D), rgb(0xE5, 0x48, 0x4D));
themed!(ACCENT_HOVER, rgb(0xD6, 0x3D, 0x42), rgb(0xD6, 0x3D, 0x42));
themed!(FRAME, rgb(0x00, 0x00, 0x00), rgb(0xE9, 0xE9, 0xEC));
themed!(SURFACE, rgb(0x12, 0x12, 0x14), rgb(0xFF, 0xFF, 0xFF));
themed!(SURFACE_2, rgb(0x19, 0x19, 0x1C), rgb(0xF6, 0xF6, 0xF8));
themed!(SURFACE_3, rgb(0x24, 0x24, 0x28), rgb(0xED, 0xED, 0xF0));
themed!(INPUT_BG, rgb(0x1C, 0x1C, 0x1F), rgb(0xF2, 0xF2, 0xF5));
themed!(TEXT, rgb(0xF5, 0xF5, 0xF7), rgb(0x1D, 0x1D, 0x1F));
themed!(TEXT_2, rgb(0xA1, 0xA1, 0xA8), rgb(0x5F, 0x5F, 0x65));
themed!(TEXT_3, rgb(0x86, 0x86, 0x8E), rgb(0x74, 0x74, 0x7A));
themed!(HOVER, rgb(0x20, 0x20, 0x23), rgb(0xF4, 0xF4, 0xF5));
themed!(ACTIVE, rgb(0x2A, 0x2A, 0x2D), rgb(0xEC, 0xEC, 0xED));
themed!(BORDER, rgb(0x2B, 0x2B, 0x2F), rgb(0xE6, 0xE6, 0xE9));
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
    pub const EDIT: char = '\u{E70F}';
    pub const BELL: char = '\u{EA8F}';
    pub const PEOPLE: char = '\u{E716}';
    pub const DEVICES: char = '\u{E772}';
    pub const SETTINGS: char = '\u{E713}';
    pub const GLOBE: char = '\u{E774}';
    pub const PLAYLIST: char = '\u{E8FD}';
    pub const CLOSE: char = '\u{E711}';
    pub const PERSON: char = '\u{E77B}';
    pub const HISTORY: char = '\u{E81C}';
    pub const LINK: char = '\u{E71B}';
    pub const BACK: char = '\u{E72B}';
    pub const SIGN_OUT: char = '\u{E7E8}';
}

pub fn icon<'a>(c: char, size: u16) -> iced::widget::Text<'a, Theme> {
    text(c.to_string()).font(ICONS).size(size as f32).line_height(1.0)
}

pub fn theme() -> Theme {
    Theme::custom(
        "Volvix Home".into(),
        iced::theme::Palette { background: FRAME(), text: TEXT(), primary: ACCENT(), success: rgb(0x30, 0xA4, 0x6C), danger: ACCENT() },
    )
}

pub fn title<'a, M: 'a>(s: impl text::IntoFragment<'a>, size: u16) -> Element<'a, M> {
    text(s).font(FONT_BOLD).size(size as f32).color(TEXT()).into()
}
pub fn dim<'a>(s: impl text::IntoFragment<'a>, size: u16) -> iced::widget::Text<'a, Theme> {
    text(s).size(size as f32).color(TEXT_2())
}

/* ---------- containers ---------- */

pub fn panel(_: &Theme) -> container::Style {
    container::Style { background: Some(SURFACE().into()), border: border::rounded(RADIUS), ..Default::default() }
}
pub fn frame(_: &Theme) -> container::Style {
    container::Style { background: Some(FRAME().into()), text_color: Some(TEXT()), ..Default::default() }
}
pub fn error_box(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Color { a: 0.14, ..ACCENT() }.into()),
        border: Border { radius: 8.0.into(), width: 1.0, color: Color { a: 0.4, ..ACCENT() } },
        text_color: Some(TEXT()),
        ..Default::default()
    }
}

/* ---------- buttons ---------- */

/// A row or tile that only shows it's clickable on hover.
pub fn flat(_: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(HOVER().into()),
        button::Status::Pressed => Some(ACTIVE().into()),
        _ => None,
    };
    button::Style { background: bg, text_color: TEXT(), border: border::rounded(8.0), ..Default::default() }
}
/// A sidebar item, highlighted when it's the current page.
pub fn nav(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |t, s| {
        let mut st = flat(t, s);
        if active {
            st.background = Some(ACTIVE().into());
        }
        st.text_color = if active { TEXT() } else { TEXT_2() };
        if matches!(s, button::Status::Hovered) {
            st.text_color = TEXT();
        }
        st
    }
}
/// Icon buttons in the player; `on` = shuffle/repeat switched on (red).
pub fn icon_btn(on: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    // the whole round area lights up on hover (not just the icon), like the website
    move |_, s| button::Style {
        background: match s {
            button::Status::Hovered => Some(HOVER().into()),
            button::Status::Pressed => Some(ACTIVE().into()),
            _ => None,
        },
        border: border::rounded(100.0),
        text_color: match (on, s) {
            (true, _) => ACCENT(),
            (false, button::Status::Hovered | button::Status::Pressed) => TEXT(),
            (false, button::Status::Disabled) => TEXT_3(),
            _ => TEXT_2(),
        },
        ..Default::default()
    }
}

/// The website's cast icon (a screen and a speaker), drawn the same way.
pub fn cast_icon<'a, M: 'a>(size: f32, color: Color) -> Element<'a, M> {
    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="14" height="10" rx="1.5"/><path d="M6 18h6"/><rect x="18" y="8" width="4" height="12" rx="1"/></svg>"#;
    iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG.as_bytes())).width(size).height(size).style(move |_: &Theme, _| iced::widget::svg::Style { color: Some(color) }).into()
}

/// The play / pause mark of the big round button: a solid triangle or two solid bars (drawn, not a font glyph,
/// so it is exactly the website's shape and sits in the middle).
pub fn play_pause<'a, M: 'a>(playing: bool, size: f32, color: Color) -> Element<'a, M> {
    const PLAY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="white" transform="translate(-1.6 0)" d="M7 4.6v14.8a1 1 0 0 0 1.5.86l12.3-7.4a1 1 0 0 0 0-1.72L8.5 3.74A1 1 0 0 0 7 4.6z"/></svg>"#;
    const PAUSE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect fill="white" x="6" y="4" width="4" height="16" rx="1"/><rect fill="white" x="14" y="4" width="4" height="16" rx="1"/></svg>"#;
    let src = if playing { PAUSE } else { PLAY };
    iced::widget::svg(iced::widget::svg::Handle::from_memory(src.as_bytes())).width(size).height(size).style(move |_: &Theme, _| iced::widget::svg::Style { color: Some(color) }).into()
}

/// A small hint that appears over a button after a moment (what the website shows as a title).
/// It appears only once the pointer has rested on the button for a moment (iced shows tooltips at once, so the
/// app tracks the pointer itself: `hint_on` / `hint_off` are called from the hover messages).
pub fn tip<'a>(content: impl Into<Element<'a, crate::Message>>, label: impl ToString) -> Element<'a, crate::Message> {
    let label = label.to_string();
    let ready = HINT.lock().map(|h| h.as_ref().map(|(l, at)| *l == label && at.elapsed() >= HINT_DELAY).unwrap_or(false)).unwrap_or(false);
    let shown: Element<'a, crate::Message> = if ready {
        // no outline: a soft shadow lifts it off the page instead
        container(text(label.clone()).size(12)).padding([5, 10]).style(|_: &Theme| container::Style {
            background: Some(SURFACE_3().into()),
            border: Border { radius: 6.0.into(), width: 0.0, color: Color::TRANSPARENT },
            text_color: Some(TEXT()),
            shadow: iced::Shadow { color: Color { a: 0.45, ..Color::BLACK }, offset: iced::Vector::new(0.0, 3.0), blur_radius: 10.0 },
            ..Default::default()
        })
        .into()
    } else {
        Space::new(0, 0).into() // nothing to show yet (the tree keeps the same shape, so the button isn't disturbed)
    };
    let watched = iced::widget::mouse_area(content).on_enter(crate::Message::HintOn(label)).on_exit(crate::Message::HintOff);
    iced::widget::tooltip(watched, shown, iced::widget::tooltip::Position::Top).gap(8).into()
}

const HINT_DELAY: std::time::Duration = std::time::Duration::from_millis(700);
static HINT: std::sync::Mutex<Option<(String, std::time::Instant)>> = std::sync::Mutex::new(None);

pub fn hint_on(label: String) {
    if let Ok(mut h) = HINT.lock() {
        *h = Some((label, std::time::Instant::now()));
    }
}

pub fn hint_off() {
    if let Ok(mut h) = HINT.lock() {
        *h = None;
    }
}
/// The big round play button.
pub fn round_play(_: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(if matches!(s, button::Status::Hovered) { rgb(0xFF, 0xFF, 0xFF) } else { TEXT() })),
        text_color: FRAME(),
        border: border::rounded(100.0),
        ..Default::default()
    }
}
pub fn primary(_: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: Some(match s {
            button::Status::Hovered | button::Status::Pressed => ACCENT_HOVER(),
            button::Status::Disabled => Color { a: 0.5, ..ACCENT() },
            _ => ACCENT(),
        }
        .into()),
        text_color: rgb(0xFF, 0xFF, 0xFF),
        border: border::rounded(100.0),
        shadow: Shadow { color: Color::TRANSPARENT, offset: Vector::ZERO, blur_radius: 0.0 },
    }
}
pub fn secondary(_: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: Some(if matches!(s, button::Status::Hovered) { SURFACE_3() } else { SURFACE_2() }.into()),
        text_color: TEXT(),
        border: Border { radius: 100.0.into(), width: 1.0, color: BORDER() },
        ..Default::default()
    }
}
/// Seconds since the app started (drives the launch screen's animation).
pub fn since_start() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
}

/// Someone's profile picture (already cut round), or the letter stand-in while there is none.
pub fn avatar_pic<'a, M: 'a>(handle: Option<&image::Handle>, name: &str, size: f32) -> Element<'a, M> {
    match handle {
        Some(h) => image(h.clone()).width(size).height(size).into(),
        None => avatar(name, size),
    }
}

/// A round picture stand-in: the first letter of a name on one of the website's avatar colours.
pub fn avatar<'a, M: 'a>(name: &str, size: f32) -> Element<'a, M> {
    const COLORS: [(u8, u8, u8); 8] = [(0x5E, 0x54, 0x8E), (0x33, 0x5C, 0x67), (0x9E, 0x2A, 0x2B), (0xBC, 0x6C, 0x25), (0x3D, 0x5A, 0x80), (0x6B, 0x4F, 0x3A), (0x4A, 0x6C, 0x2F), (0x7A, 0x40, 0x69)];
    let (r, g, b) = COLORS[name.bytes().map(|b| b as usize).sum::<usize>() % COLORS.len()];
    let initial: String = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
    container(text(initial).size(size * 0.4).font(FONT_BOLD))
        .width(size)
        .height(size)
        .center_x(size)
        .center_y(size)
        .style(move |_: &Theme| container::Style { background: Some(rgb(r, g, b).into()), border: border::rounded(100.0), text_color: Some(Color::WHITE), ..Default::default() })
        .into()
}

/// The launch screen, like the website's: the logo breathes (1 → 0.94 → 1 every 1.6 s) and "Volvix" fades in under it.
pub fn splash<'a, M: 'a>() -> Element<'a, M> {
    let t = since_start();
    let breathe = 1.0 - 0.06 * (0.5 - 0.5 * (t / 1.6 * std::f32::consts::TAU).cos());
    let word = ((t - 0.15) / 0.5).clamp(0.0, 1.0);
    let word = 1.0 - (1.0 - word).powi(2); // ease-out
    let mark = container(crate::marks::logo_scaled::<M>(104.0, breathe)).width(Length::Fill).height(Length::Fill).center_x(Length::Fill).center_y(Length::Fill);
    let name = container(text("Volvix").size(22).font(FONT_BOLD).color(Color { a: word, ..TEXT() }))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .padding(iced::Padding { top: 144.0 + (1.0 - word) * 12.0, ..Default::default() });
    // no panel colour of its own: it is the window's frame colour, the same as the title bar above it
    container(iced::widget::stack![mark, name]).width(Length::Fill).height(Length::Fill).into()
}

/// The website's segmented control: a few choices in one pill, the chosen one raised.
pub fn seg<'a>(options: &[(&'a str, &'a str)], selected: &str, f: impl Fn(String) -> crate::Message + 'a) -> Element<'a, crate::Message> {
    let mut r = iced::widget::Row::new().spacing(2).align_y(iced::Alignment::Center);
    for (value, label) in options {
        let on = *value == selected;
        r = r.push(
            button(text(*label).size(13).font(FONT_BOLD))
                .padding([6, 12])
                .style(move |_: &Theme, s: button::Status| button::Style {
                    background: if on { Some(SURFACE().into()) } else if matches!(s, button::Status::Hovered) { Some(HOVER().into()) } else { None },
                    text_color: if on || matches!(s, button::Status::Hovered) { TEXT() } else { TEXT_2() },
                    border: border::rounded(8.0),
                    shadow: if on { Shadow { color: Color { a: 0.35, ..Color::BLACK }, offset: Vector::new(0.0, 1.0), blur_radius: 2.0 } } else { Shadow::default() },
                    ..Default::default()
                })
                .on_press(f(value.to_string())),
        );
    }
    container(r).padding(3).style(|_: &Theme| container::Style { background: Some(SURFACE_3().into()), border: border::rounded(10.0), ..Default::default() }).into()
}

/// A vertical slider (the equalizer's bands): a thin rail, red below the handle, a round white handle.
pub fn vslider_style(_: &Theme, s: iced::widget::slider::Status) -> iced::widget::slider::Style {
    use iced::widget::slider::{Handle, HandleShape, Rail, Status, Style};
    let big = matches!(s, Status::Hovered | Status::Dragged);
    Style {
        rail: Rail { backgrounds: (ACCENT().into(), rgb(0x3A, 0x3A, 0x40).into()), width: 4.0, border: border::rounded(2.0) },
        handle: Handle { shape: HandleShape::Circle { radius: if big { 7.0 } else { 6.0 } }, background: Color::WHITE.into(), border_width: 0.0, border_color: Color::TRANSPARENT },
    }
}

/// A drop-down box (the equalizer's preset): dark, rounded, with the website's colours.
pub fn pick_style(_: &Theme, s: iced::widget::pick_list::Status) -> iced::widget::pick_list::Style {
    let hot = !matches!(s, iced::widget::pick_list::Status::Active);
    iced::widget::pick_list::Style {
        text_color: TEXT(),
        placeholder_color: TEXT_2(),
        handle_color: TEXT_2(),
        background: INPUT_BG().into(),
        border: Border { radius: 8.0.into(), width: 1.0, color: if hot { TEXT_2() } else { BORDER() } },
    }
}

pub fn pick_menu(_: &Theme) -> iced::overlay::menu::Style {
    iced::overlay::menu::Style { background: SURFACE_3().into(), border: Border { radius: 8.0.into(), width: 1.0, color: BORDER() }, text_color: TEXT(), selected_text_color: TEXT(), selected_background: ACTIVE().into() }
}

/// The website's switch: red when on, a white knob.
pub fn switch_style(_: &Theme, s: iced::widget::toggler::Status) -> iced::widget::toggler::Style {
    use iced::widget::toggler::Status;
    let on = matches!(s, Status::Active { is_toggled: true } | Status::Hovered { is_toggled: true });
    iced::widget::toggler::Style {
        background: if on { ACCENT() } else { rgb(0x4A, 0x4A, 0x52) },
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: Color::WHITE,
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
    }
}

/// Library tabs ("Albums", "Artists").
pub fn chip(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| button::Style {
        background: Some(if active { TEXT() } else if matches!(s, button::Status::Hovered) { SURFACE_3() } else { SURFACE_2() }.into()),
        text_color: if active { FRAME() } else { TEXT() },
        border: border::rounded(100.0),
        ..Default::default()
    }
}

pub fn input(_: &Theme, s: iced::widget::text_input::Status) -> iced::widget::text_input::Style {
    use iced::widget::text_input::{Status, Style};
    Style {
        background: INPUT_BG().into(),
        border: Border { radius: 8.0.into(), width: 1.0, color: if matches!(s, Status::Focused) { TEXT_2() } else { BORDER() } },
        icon: TEXT_2(),
        placeholder: TEXT_3(),
        value: TEXT(),
        selection: Color { a: 0.35, ..ACCENT() },
    }
}

pub fn scroller(_: &Theme, s: iced::widget::scrollable::Status) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{Rail, Scroller, Status, Style};
    let shown = !matches!(s, Status::Active);
    let rail = Rail {
        background: None,
        border: border::rounded(4.0),
        scroller: Scroller { color: if shown { Color { a: 0.25, ..TEXT() } } else { Color::TRANSPARENT }, border: border::rounded(4.0) },
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
            container(mark.size(size * 0.38).color(Color { a: 0.85, ..rgb(0xF5, 0xF5, 0xF7) }))
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
    container::Style { background: Some(Color { a: 0.6, ..FRAME() }.into()), ..Default::default() }
}
/// A dialog's card.
pub fn dialog(_: &Theme) -> container::Style {
    container::Style {
        background: Some(SURFACE_2().into()),
        border: Border { radius: 14.0.into(), width: 1.0, color: BORDER() },
        text_color: Some(TEXT()),
        shadow: Shadow { color: Color { a: 0.5, ..FRAME() }, offset: Vector::new(0.0, 12.0), blur_radius: 32.0 },
    }
}
/// The little message that pops up above the player.
pub fn toast(_: &Theme) -> container::Style {
    container::Style { background: Some(TEXT().into()), text_color: Some(FRAME()), border: border::rounded(8.0), ..Default::default() }
}
/// The unread count on the bell.
pub fn badge(_: &Theme) -> container::Style {
    container::Style { background: Some(ACCENT().into()), text_color: Some(rgb(0xFF, 0xFF, 0xFF)), border: border::rounded(9.0), ..Default::default() }
}
/// Mix cards: a coloured tile per mix.
pub fn mix_tile(seed: &str) -> impl Fn(&Theme) -> container::Style {
    let tint = tint_for(seed);
    move |_| container::Style {
        background: Some(Background::Gradient(iced::Gradient::Linear(iced::gradient::Linear::new(0.8).add_stop(0.0, Color { a: 1.0, ..ACCENT() }).add_stop(1.0, tint)))),
        border: border::rounded(6.0),
        text_color: Some(rgb(0xF5, 0xF5, 0xF7)), // on a coloured tile the text is light in both themes
        ..Default::default()
    }
}

pub fn gap<'a, M: 'a>(px: u16) -> Element<'a, M> {
    Space::with_height(Length::Fixed(px as f32)).into()
}
