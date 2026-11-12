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
// the stronger outline (round buttons): white / black at 16% over the panel
themed!(BORDER_STRONG, rgb(0x38, 0x38, 0x3A), rgb(0xD6, 0xD6, 0xD6));
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
    pub const BELL: char = '\u{EA8F}';
    pub const PEOPLE: char = '\u{E716}';
    pub const SETTINGS: char = '\u{E713}';
    pub const PLAYLIST: char = '\u{E8FD}';
    pub const CLOSE: char = '\u{E711}';
    pub const PERSON: char = '\u{E77B}';
    pub const LINK: char = '\u{E71B}';
    pub const MORE: char = '\u{E712}';
    pub const BACK: char = '\u{E72B}';
}

/// The website's icon by the name it has there (`S.icon(name, size)` in web/js/ui.js), in an explicit colour. On
/// Windows the website shows the system glyph for some names and its own drawing for the rest; this shows the same one.
pub fn wi<'a, M: 'a>(name: &str, size: f32, color: Color) -> Element<'a, M> {
    if let Some((_, c)) = crate::webicons::GLYPHS.iter().find(|(n, _)| *n == name) {
        return container(text(c.to_string()).font(ICONS).size((size * 0.9).round()).color(color).line_height(1.0)).width(size).height(size).center_x(size).center_y(size).into();
    }
    match web_svg(name) {
        Some(h) => iced::widget::svg(h).width(size).height(size).style(move |_: &Theme, _| iced::widget::svg::Style { color: Some(color) }).into(),
        None => Space::new(size, size).into(),
    }
}

/// The same icon inside a button: it takes the button's colour (grey, lighter under the pointer, red when `on`).
pub fn wi_btn<'a, M: 'a>(name: &str, size: f32, on: bool) -> Element<'a, M> {
    if let Some((_, c)) = crate::webicons::GLYPHS.iter().find(|(n, _)| *n == name) {
        // a glyph is text: it follows the button's text colour by itself
        return container(text(c.to_string()).font(ICONS).size((size * 0.9).round()).line_height(1.0)).width(size).height(size).center_x(size).center_y(size).into();
    }
    match web_svg(name) {
        Some(h) => iced::widget::svg(h)
            .width(size)
            .height(size)
            .style(move |_: &Theme, s| iced::widget::svg::Style { color: Some(if on { ACCENT() } else if matches!(s, iced::widget::svg::Status::Hovered) { TEXT() } else { TEXT_2() }) })
            .into(),
        None => Space::new(size, size).into(),
    }
}

/// One of the website's drawn icons as a picture (made once, then kept).
fn web_svg(name: &str) -> Option<iced::widget::svg::Handle> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, iced::widget::svg::Handle>>> = std::sync::OnceLock::new();
    let mut cache = CACHE.get_or_init(Default::default).lock().ok()?;
    if let Some(h) = cache.get(name) {
        return Some(h.clone());
    }
    let body = crate::webicons::ICONS.iter().find(|(n, _)| *n == name)?.1;
    let doc = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{body}</svg>"#);
    let h = iced::widget::svg::Handle::from_memory(doc.into_bytes());
    cache.insert(name.to_string(), h.clone());
    Some(h)
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
/// The sound-quality mark of the player bar: five bars, like a waveform.
pub fn quality_icon<'a, M: 'a>(size: f32, color: Color) -> Element<'a, M> {
    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round"><path d="M4 10v4M8 6v12M12 3v18M16 8v8M20 11v2"/></svg>"#;
    iced::widget::svg(iced::widget::svg::Handle::from_memory(SVG.as_bytes())).width(size).height(size).style(move |_: &Theme, _| iced::widget::svg::Style { color: Some(color) }).into()
}

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
/// The website's secondary button: a grey pill with a hairline around it.
pub fn secondary(_: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: Some(if matches!(s, button::Status::Hovered | button::Status::Pressed) { ACTIVE() } else { SURFACE_3() }.into()),
        text_color: if matches!(s, button::Status::Disabled) { TEXT_3() } else { TEXT() },
        border: Border { radius: 100.0.into(), width: 1.0, color: BORDER() },
        ..Default::default()
    }
}

/// The website's ghost button: just words, lighter (with a soft background) under the pointer.
pub fn ghost(_: &Theme, s: button::Status) -> button::Style {
    let hot = matches!(s, button::Status::Hovered | button::Status::Pressed);
    button::Style { background: hot.then(|| HOVER().into()), text_color: if hot { TEXT() } else { TEXT_2() }, border: border::rounded(100.0), ..Default::default() }
}

/// A red button for something that can't be undone.
pub fn danger(_: &Theme, s: button::Status) -> button::Style {
    button::Style {
        background: Some(if matches!(s, button::Status::Hovered | button::Status::Pressed) { rgb(0xC2, 0x28, 0x2E) } else { rgb(0xD9, 0x30, 0x36) }.into()),
        text_color: Color::WHITE,
        border: border::rounded(100.0),
        ..Default::default()
    }
}

/// Words that act as a link (a song's album, its artist): no background, even under the pointer.
pub fn link(_: &Theme, _: button::Status) -> button::Style {
    button::Style { background: None, text_color: TEXT(), ..Default::default() }
}

/// A text link: `base` colour, and it lights up (and the pointer becomes a hand) when hovered, like the website's links.
pub fn link_hover(base: Color, lit: Color) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| button::Style { background: None, text_color: if matches!(s, button::Status::Hovered | button::Status::Pressed) { lit } else { base }, ..Default::default() }
}

/// A round outlined button (the website's 40–42 px ones on album, playlist and Now Playing pages); red when `on`.
pub fn ring_btn(on: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| button::Style {
        background: matches!(s, button::Status::Hovered | button::Status::Pressed).then(|| HOVER().into()),
        text_color: if on {
            ACCENT()
        } else if matches!(s, button::Status::Disabled) {
            TEXT_3()
        } else {
            TEXT()
        },
        border: Border { radius: 100.0.into(), width: 1.0, color: BORDER_STRONG() },
        ..Default::default()
    }
}

/// Which of the website's buttons.
#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

fn btn_style(kind: Btn) -> fn(&Theme, button::Status) -> button::Style {
    match kind {
        Btn::Primary => primary,
        Btn::Secondary => secondary,
        Btn::Ghost => ghost,
        Btn::Danger => danger,
    }
}

/// The website's button: a pill 40 tall with 14 px words.
pub fn btn<'a>(label: impl text::IntoFragment<'a>, kind: Btn) -> iced::widget::Button<'a, crate::Message> {
    button(text(label).size(14).wrapping(text::Wrapping::None)).padding([10.9, 18.0]).style(btn_style(kind))
}

/// The website's small button: 32 tall with 13 px words.
pub fn btn_sm<'a>(label: impl text::IntoFragment<'a>, kind: Btn) -> iced::widget::Button<'a, crate::Message> {
    button(text(label).size(13).wrapping(text::Wrapping::None)).padding([7.55, 14.0]).style(btn_style(kind))
}

/// The same with one of the website's icons in front of the words.
pub fn btn_icon<'a>(icon_name: &str, label: impl text::IntoFragment<'a>, kind: Btn, small: bool) -> iced::widget::Button<'a, crate::Message> {
    let color = if kind == Btn::Ghost { TEXT_2() } else if matches!(kind, Btn::Primary | Btn::Danger) { Color::WHITE } else { TEXT() };
    let (size, icon_size, pad) = if small { (13.0, 14.0, [7.55, 14.0]) } else { (14.0, 16.0, [10.9, 18.0]) };
    button(iced::widget::row![wi(icon_name, icon_size, color), text(label).size(size).wrapping(text::Wrapping::None)].spacing(if small { 6 } else { 8 }).align_y(iced::Alignment::Center)).padding(pad).style(btn_style(kind))
}
/// Seconds since the app started (drives the launch screen's animation).
pub fn since_start() -> f32 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f32()
}

/// Someone's profile picture (already cut round), or the letter stand-in while there is none.
pub fn avatar_pic<'a, M: 'a>(handle: Option<&image::Handle>, id: &str, name: &str, size: f32) -> Element<'a, M> {
    match handle {
        Some(h) => image(h.clone()).width(size).height(size).into(),
        None => avatar_key(id, name, size),
    }
}

/// Same colour and letters as the website: the colour comes from the person's id (or name), the letters from the first two words of the name.
pub fn avatar_key<'a, M: 'a>(key: &str, name: &str, size: f32) -> Element<'a, M> {
    const COLORS: [(u8, u8, u8); 8] = [(0x5E, 0x54, 0x8E), (0x33, 0x5C, 0x67), (0x9E, 0x2A, 0x2B), (0xBC, 0x6C, 0x25), (0x3D, 0x5A, 0x80), (0x6B, 0x4F, 0x3A), (0x4A, 0x6C, 0x2F), (0x7A, 0x40, 0x69)];
    let (r, g, b) = COLORS[(key.chars().fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32)) as usize) % COLORS.len()];
    let initial: String = name.split_whitespace().filter_map(|p| p.chars().next()).take(2).collect::<String>().to_uppercase();
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
    cover_box(handle, title, size, size, 6.0)
}

/// Art with no picture is a tinted tile with its name on it (the same colour per name as the website). Big tiles
/// print the whole name; tiny ones (a list row) just the first letter. `w` × `h`: a collage uses parts of a square.
pub fn cover_box<'a, M: 'a>(handle: Option<&image::Handle>, title: &str, w: f32, h: f32, radius: f32) -> Element<'a, M> {
    if let Some(handle) = handle {
        return image(handle.clone()).width(w).height(h).content_fit(iced::ContentFit::Cover).into();
    }
    let tint = tint_for(title);
    let ink = Color { a: 0.92, ..rgb(0xF5, 0xF5, 0xF7) };
    let name = title.trim();
    let letter = name.chars().find(|c| c.is_alphanumeric());
    let side = w.min(h);
    let body: Element<'a, M> = match letter {
        None => wi("music", if side >= 100.0 { side * 0.38 } else { 22.0 }, ink), // the website's note, when there's no title
        Some(c) if side <= 76.0 => text(c.to_uppercase().collect::<String>()).font(FONT_BOLD).size(side * 0.4).color(ink).into(),
        Some(_) => {
            // the website's sizes: shorter names are set bigger
            let n = name.chars().count();
            let pct = if n <= 10 { 0.17 } else if n <= 22 { 0.13 } else if n <= 40 { 0.105 } else { 0.085 };
            text(name.to_string())
                .font(FONT_BOLD)
                .size(side * pct)
                .color(ink)
                .align_x(iced::alignment::Horizontal::Center)
                .width(Length::Fixed(w * 0.82))
                .wrapping(text::Wrapping::WordOrGlyph)
                .into()
        }
    };
    container(body)
        .width(w)
        .height(h)
        .center_x(w)
        .center_y(h)
        .clip(true)
        .style(move |_| container::Style { background: Some(tint.into()), border: border::rounded(radius), ..Default::default() })
        .into()
}

/// The website's drawn-cover backgrounds: (name, kind, primary, secondary, text colour, gradient angle). Your own
/// colours replace the primary and secondary ones; the kind decides how they are used.
const LOOKS: [(&str, &str, u32, u32, u32, f32); 16] = [
    ("sunset", "linear", 0xff6a3d, 0xffd89b, 0x3a1200, 135.0),
    ("ocean", "linear", 0x0f4c75, 0x7fd1e8, 0xffffff, 160.0),
    ("midnight", "linear", 0x0b132b, 0x3a506b, 0xe8f1ff, 160.0),
    ("forest", "linear", 0x0f3d2e, 0x8fc98a, 0xf3fff0, 150.0),
    ("grape", "linear", 0x3c1053, 0xc99bff, 0xffffff, 145.0),
    ("rose", "linear", 0xffd1dc, 0xff6f91, 0x4a0f22, 145.0),
    ("ember", "linear", 0xffcf5a, 0x8a1c0d, 0xffffff, 135.0),
    ("mono", "linear", 0x2b2b2e, 0x55555c, 0xf4f4f5, 160.0),
    ("paper", "solid", 0xf1e9db, 0xf1e9db, 0x2a241c, 0.0),
    ("stripes", "stripes", 0xe5484d, 0xc23a3f, 0xffffff, 0.0),
    ("rings", "rings", 0x1d3557, 0x274b7a, 0xf1faee, 0.0),
    ("grid", "grid", 0x101820, 0x3a4650, 0xe7f6ff, 0.0),
    ("sunburst", "sunburst", 0xffd23f, 0xff9f1c, 0x2b1800, 0.0),
    ("checker", "checker", 0x222222, 0xeeeeee, 0xffffff, 0.0),
    ("aurora", "linear", 0x0d1b2a, 0xe0a1ff, 0xffffff, 200.0),
    ("dusk", "linear", 0x2b1055, 0xffc3a0, 0xffffff, 180.0),
];

/// "#rgb" or "#rrggbb" as a colour (None when it isn't one).
pub fn hex(s: &str) -> Option<Color> {
    let h = s.trim().strip_prefix('#')?;
    let v = |x: &str| u8::from_str_radix(x, 16).ok();
    match h.len() {
        3 => Some(rgb(v(&h[0..1])? * 17, v(&h[1..2])? * 17, v(&h[2..3])? * 17)),
        6 | 8 => Some(rgb(v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?)),
        _ => None,
    }
}

pub fn to_hex(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", (c.r * 255.0).round() as u8, (c.g * 255.0).round() as u8, (c.b * 255.0).round() as u8)
}

/// What a look draws with: (kind, primary, secondary, text colour, angle), using the preset's colours where yours are absent.
pub fn look_parts(look: &volvix_core::Look) -> (&'static str, Color, Color, Color, f32) {
    let p = LOOKS.iter().find(|l| l.0 == look.bg).unwrap_or(&LOOKS[15]);
    let h = |x: u32| rgb((x >> 16) as u8, (x >> 8) as u8, x as u8);
    let own = |v: &Option<String>, d: u32| v.as_deref().and_then(hex).unwrap_or_else(|| h(d));
    (p.1, own(&look.c1, p.2), own(&look.c2, p.3), own(&look.ink, p.4), p.5)
}

/// The preset's own primary, secondary and text colours (what the colour boxes show until you pick one).
pub fn look_defaults(bg: &str) -> (Color, Color, Color) {
    let p = LOOKS.iter().find(|l| l.0 == bg).unwrap_or(&LOOKS[15]);
    let h = |x: u32| rgb((x >> 16) as u8, (x >> 8) as u8, x as u8);
    (h(p.2), h(p.3), h(p.4))
}

fn mix(a: Color, b: Color) -> Color {
    Color { r: (a.r + b.r) / 2.0, g: (a.g + b.g) / 2.0, b: (a.b + b.b) / 2.0, a: 1.0 }
}

/// The patterned backgrounds, drawn as a picture in your two colours (stripes, rings, grid, sunburst, checker).
fn look_svg(kind: &str, c1: Color, c2: Color, rx: f32) -> Option<String> {
    let (h1, h2) = (to_hex(c1), to_hex(c2));
    let body = match kind {
        "stripes" => {
            let mut s = format!("<rect width='100' height='100' fill='{h1}'/><g transform='rotate(45 50 50)'>");
            for i in -10..20 {
                if i % 2 != 0 {
                    s += &format!("<rect x='-50' y='{}' width='200' height='7' fill='{h2}'/>", i * 7);
                }
            }
            s + "</g>"
        }
        "rings" => {
            let mut s = format!("<rect width='100' height='100' fill='{h1}'/>");
            for i in (0..16).rev() {
                s += &format!("<circle cx='70' cy='30' r='{}' fill='{}'/>", (i + 1) * 6, if i % 2 == 0 { &h2 } else { &h1 });
            }
            s
        }
        "grid" => {
            let mut s = format!("<rect width='100' height='100' fill='{h1}'/>");
            for i in 0..10 {
                let p = i as f32 * 11.0;
                s += &format!("<path d='M{p} 0V100M0 {p}H100' stroke='{h2}' stroke-width='0.5'/>");
            }
            s
        }
        "sunburst" => {
            let mut s = format!("<rect width='100' height='100' fill='{h1}'/>");
            for i in 0..18 {
                let (a0, a1) = (((i * 20 + 10) as f32).to_radians(), ((i * 20 + 20) as f32).to_radians());
                s += &format!("<path d='M50 38L{:.1} {:.1}L{:.1} {:.1}Z' fill='{h2}'/>", 50.0 + 200.0 * a0.sin(), 38.0 - 200.0 * a0.cos(), 50.0 + 200.0 * a1.sin(), 38.0 - 200.0 * a1.cos());
            }
            s
        }
        "checker" => {
            let mut s = format!("<rect width='100' height='100' fill='{h2}'/>");
            for y in 0..10 {
                for x in 0..10 {
                    if (x + y) % 2 == 0 {
                        s += &format!("<rect x='{}' y='{}' width='10' height='10' fill='{h1}'/>", x * 10, y * 10);
                    }
                }
            }
            s
        }
        _ => return None,
    };
    Some(format!("<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100' preserveAspectRatio='xMidYMid slice'><clipPath id='c'><rect width='100' height='100' rx='{rx:.2}'/></clipPath><g clip-path='url(#c)'>{body}</g></svg>"))
}

/// The website's cover fonts, as far as Windows has them.
pub fn look_font(name: &str) -> (Font, bool) {
    let f = |n: &'static str| Font { weight: iced::font::Weight::Bold, ..Font::with_name(n) };
    match name {
        "serif" => (f("Georgia"), false),
        "mono" => (f("Consolas"), false),
        "rounded" => (f("Segoe UI"), false),
        "condensed" => (f("Impact"), true),
        "script" => (Font { weight: iced::font::Weight::Normal, ..Font::with_name("Segoe Script") }, false),
        "slab" => (f("Rockwell"), false),
        _ => (FONT_BOLD, false),
    }
}

/// A drawn cover in the look someone chose (background, colours, font) for an album or playlist.
pub fn cover_look<'a, M: 'a>(title: &str, size: f32, radius: f32, look: &volvix_core::Look) -> Element<'a, M> {
    let (kind, c1, c2, ink, angle) = look_parts(look);
    let (font, upper) = look_font(&look.font);
    let name = title.trim();
    let shown = if upper { name.to_uppercase() } else { name.to_string() };
    let body: Element<'a, M> = if name.is_empty() {
        Space::new(0, 0).into()
    } else if size <= 76.0 {
        text(name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default()).font(font).size(size * 0.4).color(ink).into()
    } else {
        let n = name.chars().count();
        let pct = if n <= 10 { 0.17 } else if n <= 22 { 0.13 } else if n <= 40 { 0.105 } else { 0.085 };
        text(shown).font(font).size(size * pct).color(ink).align_x(iced::alignment::Horizontal::Center).width(Length::Fixed(size * 0.82)).wrapping(text::Wrapping::WordOrGlyph).into()
    };
    if let Some(doc) = look_svg(kind, c1, c2, radius / size * 100.0) {
        let art = iced::widget::svg(iced::widget::svg::Handle::from_memory(doc.into_bytes())).width(size).height(size).content_fit(iced::ContentFit::Cover);
        return iced::widget::stack![art, container(body).width(size).height(size).center_x(size).center_y(size)].width(size).height(size).into();
    }
    let background: Background = if kind == "solid" {
        c1.into()
    } else {
        Background::Gradient(iced::Gradient::Linear(iced::gradient::Linear::new(angle.to_radians()).add_stop(0.0, c1).add_stop(0.55, mix(c1, c2)).add_stop(1.0, c2)))
    };
    container(body)
        .width(size)
        .height(size)
        .center_x(size)
        .center_y(size)
        .clip(true)
        .style(move |_| container::Style { background: Some(background), border: border::rounded(radius), ..Default::default() })
        .into()
}

/// The artist page's big round stand-in: the artist's first album's tint with their initials.
pub fn artist_hero<'a, M: 'a>(name: &str, tint_of: &str, size: f32) -> Element<'a, M> {
    let initials: String = name.split_whitespace().filter(|w| w.chars().any(|c| c.is_alphanumeric())).filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase();
    let bg = tint_for(tint_of);
    container(text(initials).size(size * 0.36).font(FONT_BOLD).color(Color { a: 0.92, ..rgb(0xF5, 0xF5, 0xF7) }))
        .width(size)
        .height(size)
        .center_x(size)
        .center_y(size)
        .style(move |_: &Theme| container::Style { background: Some(bg.into()), border: border::rounded(size * 0.1), ..Default::default() })
        .into()
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
