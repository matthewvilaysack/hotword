//! City-783: rain-black surfaces, cool gray text, hard red for the signal.

use ratatui::style::{Color, Modifier, Style};

pub const BG: Color = Color::Rgb(0x18, 0x1a, 0x1f);
pub const PANEL: Color = Color::Rgb(0x20, 0x23, 0x2a);
pub const SELECTION: Color = Color::Rgb(0x2b, 0x2f, 0x37);
pub const MUTED: Color = Color::Rgb(0x4b, 0x51, 0x5b);
pub const DIM: Color = Color::Rgb(0x8f, 0x94, 0x9c);
pub const FG: Color = Color::Rgb(0xb9, 0xbe, 0xc6);
pub const BRIGHT: Color = Color::Rgb(0xec, 0xef, 0xf2);
pub const RED: Color = Color::Rgb(0xad, 0x22, 0x22);
pub const SIGNAL: Color = Color::Rgb(0xff, 0x5c, 0x5c);
pub const CYAN: Color = Color::Rgb(0xaa, 0xb0, 0xb9);

pub fn base() -> Style {
    Style::default().fg(FG).bg(BG)
}

pub fn border(focused: bool) -> Style {
    Style::default().fg(if focused { SIGNAL } else { MUTED })
}

pub fn title(focused: bool) -> Style {
    Style::default()
        .fg(if focused { BRIGHT } else { DIM })
        .add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default()
        .fg(BRIGHT)
        .bg(SELECTION)
        .add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(DIM)
}

pub fn bright() -> Style {
    Style::default().fg(BRIGHT)
}

pub fn signal() -> Style {
    Style::default().fg(SIGNAL)
}

pub fn key() -> Style {
    Style::default().fg(SIGNAL).add_modifier(Modifier::BOLD)
}

pub fn status(status: &str) -> Style {
    match status {
        "ok" => Style::default().fg(BRIGHT),
        "fail" | "timeout" => Style::default().fg(SIGNAL),
        "running" => Style::default().fg(CYAN),
        _ => Style::default().fg(DIM),
    }
}
