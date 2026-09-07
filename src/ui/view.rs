//! Draws the state. Three panels in the lazygit shape: workflows and steps on
//! the left, the report on the right, keys along the bottom.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use super::state::{Focus, Mode, State};
use super::theme;
use crate::report::render_text;
use crate::runner::Status;
use crate::workflow::Store;

pub fn draw(frame: &mut Frame, state: &State, store: &Store) {
    let area = frame.area();
    frame.render_widget(Block::default().style(theme::base()), area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);
    draw_header(frame, rows[0], state, store);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(rows[1]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(columns[0]);
    draw_workflows(frame, left[0], state);
    draw_steps(frame, left[1], state);
    draw_report(frame, columns[1], state);
    draw_footer(frame, rows[2], state);
    if state.mode == Mode::Help {
        draw_help(frame, area);
    }
}

fn draw_header(frame: &mut Frame, area: Rect, state: &State, store: &Store) {
    let running = state
        .all
        .iter()
        .filter(|l| state.is_running(&l.workflow.name))
        .count();
    let mut spans = vec![
        Span::styled(" hotword ", theme::key()),
        Span::styled(format!("{} workflows", state.all.len()), theme::bright()),
        Span::styled(
            format!("  user {}", collapse(&store.user_dir.display().to_string())),
            theme::dim(),
        ),
    ];
    match &store.project_dir {
        Some(dir) => spans.push(Span::styled(
            format!("  project {}", collapse(&dir.display().to_string())),
            theme::dim(),
        )),
        None => spans.push(Span::styled("  project none", theme::dim())),
    }
    if running > 0 {
        spans.push(Span::styled(
            format!("  {running} running"),
            theme::status("running"),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_workflows(frame: &mut Frame, area: Rect, state: &State) {
    let focused = state.focus == Focus::Workflows;
    let visible = state.visible();
    let items: Vec<ListItem> = visible
        .iter()
        .map(|l| {
            let name = &l.workflow.name;
            let badge = if state.is_running(name) {
                Span::styled(" running", theme::status("running"))
            } else if let Some(report) = state.report(name) {
                let failed = report.count(Status::Fail) + report.count(Status::Timeout);
                if failed > 0 {
                    Span::styled(format!(" {failed} failed"), theme::status("fail"))
                } else {
                    Span::styled(
                        format!(" {} ok", report.count(Status::Ok)),
                        theme::status("ok"),
                    )
                }
            } else {
                Span::styled("", theme::dim())
            };
            let src = if l.source == crate::workflow::Source::Project {
                "project"
            } else {
                "user"
            };
            ListItem::new(vec![
                Line::from(vec![Span::styled(name.clone(), theme::bright()), badge]),
                Line::from(Span::styled(
                    format!("  {}  {}", l.workflow.triggers.join(" | "), src),
                    theme::dim(),
                )),
            ])
        })
        .collect();
    let title = if state.filter.is_empty() {
        " Workflows ".to_string()
    } else {
        format!(" Workflows /{} ", state.filter)
    };
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border(focused))
                .title(Span::styled(title, theme::title(focused))),
        )
        .highlight_style(theme::selected());
    let mut list_state = ListState::default();
    if !visible.is_empty() {
        list_state.select(Some(state.cursor()));
    }
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn draw_steps(frame: &mut Frame, area: Rect, state: &State) {
    let focused = state.focus == Focus::Steps;
    let mut lines: Vec<Line> = Vec::new();
    if let Some(loaded) = state.selected() {
        let name = &loaded.workflow.name;
        let done = state
            .report(name)
            .map(|r| r.steps.clone())
            .unwrap_or_default();
        let partial = state.partial_steps(name);
        for step in &loaded.workflow.steps {
            let result = partial
                .iter()
                .chain(done.iter())
                .find(|r| r.name == step.name);
            let (mark, status, ms) = match result {
                Some(r) => (
                    match r.status {
                        Status::Ok => "●",
                        Status::Skip => "○",
                        _ => "✕",
                    },
                    r.status.as_str().to_string(),
                    format!("{}ms", r.ms),
                ),
                None if state.is_running(name) => ("…", "running".to_string(), String::new()),
                None => ("·", String::new(), String::new()),
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {mark} "), theme::status(&status)),
                Span::styled(step.name.clone(), theme::bright()),
                Span::styled(format!("  {status} {ms}"), theme::status(&status)),
            ]));
            lines.push(Line::from(Span::styled(
                format!(
                    "     {}",
                    truncate(&step.run, area.width.saturating_sub(7) as usize)
                ),
                theme::dim(),
            )));
        }
    }
    let title = match state.selected() {
        Some(l) => match state.progress(&l.workflow.name) {
            Some((done, total)) => format!(" Steps {done}/{total} "),
            None => format!(" Steps ({}) ", l.workflow.steps.len()),
        },
        None => " Steps ".to_string(),
    };
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border(focused))
                .title(Span::styled(title, theme::title(focused))),
        ),
        area,
    );
}

fn draw_report(frame: &mut Frame, area: Rect, state: &State) {
    let focused = state.focus == Focus::Report;
    let (title, body): (String, String) = match state.selected() {
        Some(l) if state.is_running(&l.workflow.name) => {
            let steps = state.partial_steps(&l.workflow.name);
            let mut text = format!("running {}\n\n", l.workflow.name);
            for s in steps {
                text.push_str(&format!("{} ({}, {}ms)\n", s.name, s.status.as_str(), s.ms));
                for line in s.output.lines().take(l.workflow.max_lines) {
                    text.push_str(&format!("  {line}\n"));
                }
                if let Some(reason) = &s.reason {
                    text.push_str(&format!("  {reason}\n"));
                }
                text.push('\n');
            }
            (format!(" Report: {} ", l.workflow.name), text)
        }
        Some(l) => match state.report(&l.workflow.name) {
            Some(report) => (
                format!(" Report: {} ", l.workflow.name),
                render_text(report, true),
            ),
            None => (
                " Report ".to_string(),
                format!(
                    "{}\n\n{}\n\nPress enter to run it, or p to run it with a prompt.",
                    l.workflow.name,
                    l.workflow
                        .description
                        .clone()
                        .unwrap_or_else(|| "No description.".into())
                ),
            ),
        },
        None => (
            " Report ".to_string(),
            "No workflows match. Press esc to clear the filter.".to_string(),
        ),
    };
    let lines: Vec<Line> = body
        .lines()
        .map(|line| {
            let style =
                if line.starts_with("hotword:") || line.ends_with(':') && !line.starts_with(' ') {
                    theme::bright()
                } else if line.contains(",fail,")
                    || line.contains(",timeout,")
                    || line.contains("(exit ")
                {
                    theme::signal()
                } else if line.starts_with("help:") || line.contains("skipped,") {
                    theme::dim()
                } else {
                    Style::default().fg(theme::FG)
                };
            Line::from(Span::styled(line.to_string(), style))
        })
        .collect();
    let max_scroll = lines
        .len()
        .saturating_sub(area.height.saturating_sub(2) as usize);
    let scroll = state.report_scroll.min(max_scroll) as u16;
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::border(focused))
                    .title(Span::styled(title, theme::title(focused))),
            ),
        area,
    );
}

fn draw_footer(frame: &mut Frame, area: Rect, state: &State) {
    let line = match state.mode {
        Mode::Filter => Line::from(vec![
            Span::styled(" / ", theme::key()),
            Span::styled(format!("{}▎", state.filter), theme::bright()),
            Span::styled("  enter keep, esc clear", theme::dim()),
        ]),
        Mode::Prompt => Line::from(vec![
            Span::styled(" prompt ", theme::key()),
            Span::styled(format!("{}▎", state.prompt), theme::bright()),
            Span::styled(
                "  enter run with HOTWORD_PROMPT set, esc cancel",
                theme::dim(),
            ),
        ]),
        _ => {
            if let Some(message) = &state.message {
                Line::from(vec![
                    Span::styled(" ! ", theme::key()),
                    Span::styled(message.clone(), theme::signal()),
                ])
            } else {
                keys(&[
                    ("enter", "run"),
                    ("p", "prompt"),
                    ("/", "filter"),
                    ("e", "edit"),
                    ("R", "refresh"),
                    ("tab", "panel"),
                    ("j/k", "move"),
                    ("?", "help"),
                    ("q", "quit"),
                ])
            }
        }
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn keys(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (k, v) in pairs {
        spans.push(Span::styled(format!(" {k} "), theme::key()));
        spans.push(Span::styled((*v).to_string(), theme::dim()));
    }
    Line::from(spans)
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let width = 56.min(area.width.saturating_sub(4));
    let height = 16.min(area.height.saturating_sub(2));
    let popup = Rect {
        x: (area.width - width) / 2,
        y: (area.height - height) / 2,
        width,
        height,
    };
    let rows = [
        ("j / k, ↑ / ↓", "move, or scroll the report"),
        ("g / G", "top / bottom"),
        ("tab, l / h", "next / previous panel"),
        ("enter", "run the selected workflow"),
        ("p", "run it with a prompt (HOTWORD_PROMPT)"),
        ("/", "filter by name or phrase"),
        ("e", "open the workflow file in $EDITOR"),
        ("R", "reload workflows from disk"),
        ("esc", "clear the filter or message"),
        ("q", "quit"),
    ];
    let lines: Vec<Line> = rows
        .iter()
        .map(|(k, v)| {
            Line::from(vec![
                Span::styled(format!(" {k:<16}"), theme::key()),
                Span::styled((*v).to_string(), theme::bright()),
            ])
        })
        .collect();
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(theme::PANEL))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::border(true))
                    .title(Span::styled(" Keys (any key closes) ", theme::title(true))),
            ),
        popup,
    );
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        let cut: String = text.chars().take(width.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

fn collapse(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_string(),
    }
}
