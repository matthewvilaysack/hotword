//! Everything the terminal UI knows, with no terminal in it. Keys become
//! `Action`s, `State::handle` applies them, and the caller decides what to run.

use std::collections::{BTreeMap, HashSet};

use crate::runner::{Report, StepResult};
use crate::workflow::Loaded;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Workflows,
    Steps,
    Report,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Filter,
    Prompt,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Up,
    Down,
    Top,
    Bottom,
    NextPanel,
    PrevPanel,
    Run,
    StartPrompt,
    StartFilter,
    Type(char),
    Backspace,
    Confirm,
    Cancel,
    Help,
    Refresh,
    Quit,
}

/// A run the caller should start: the workflow name and an optional prompt.
pub type RunRequest = (String, Option<String>);

#[derive(Debug)]
pub struct State {
    pub all: Vec<Loaded>,
    pub focus: Focus,
    pub mode: Mode,
    pub filter: String,
    pub prompt: String,
    pub report_scroll: usize,
    pub should_quit: bool,
    pub wants_refresh: bool,
    pub message: Option<String>,
    cursor: usize,
    running: HashSet<String>,
    partial: BTreeMap<String, Vec<StepResult>>,
    reports: BTreeMap<String, Report>,
}

impl State {
    pub fn new(all: Vec<Loaded>) -> State {
        State {
            all,
            focus: Focus::Workflows,
            mode: Mode::Normal,
            filter: String::new(),
            prompt: String::new(),
            report_scroll: 0,
            should_quit: false,
            wants_refresh: false,
            message: None,
            cursor: 0,
            running: HashSet::new(),
            partial: BTreeMap::new(),
            reports: BTreeMap::new(),
        }
    }

    pub fn replace_workflows(&mut self, all: Vec<Loaded>) {
        self.all = all;
        self.clamp();
    }

    /// Workflows matching the filter, by name or trigger phrase.
    pub fn visible(&self) -> Vec<&Loaded> {
        let needle = self.filter.to_lowercase();
        self.all
            .iter()
            .filter(|l| {
                needle.is_empty()
                    || l.workflow.name.to_lowercase().contains(&needle)
                    || l.workflow
                        .triggers
                        .iter()
                        .any(|t| t.to_lowercase().contains(&needle))
            })
            .collect()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn selected(&self) -> Option<&Loaded> {
        self.visible().get(self.cursor).copied()
    }

    pub fn is_running(&self, name: &str) -> bool {
        self.running.contains(name)
    }

    pub fn mark_running(&mut self, name: &str) {
        self.running.insert(name.to_string());
        self.partial.insert(name.to_string(), Vec::new());
    }

    pub fn push_step(&mut self, name: &str, step: StepResult) {
        self.partial.entry(name.to_string()).or_default().push(step);
    }

    /// (steps finished, steps total) while a workflow runs.
    pub fn progress(&self, name: &str) -> Option<(usize, usize)> {
        if !self.running.contains(name) {
            return None;
        }
        let total = self
            .all
            .iter()
            .find(|l| l.workflow.name == name)?
            .workflow
            .steps
            .len();
        Some((self.partial.get(name).map_or(0, Vec::len), total))
    }

    pub fn partial_steps(&self, name: &str) -> &[StepResult] {
        self.partial.get(name).map_or(&[], Vec::as_slice)
    }

    pub fn finish(&mut self, name: &str, report: Report) {
        self.running.remove(name);
        self.partial.remove(name);
        self.reports.insert(name.to_string(), report);
    }

    pub fn fail_run(&mut self, name: &str, message: String) {
        self.running.remove(name);
        self.partial.remove(name);
        self.message = Some(message);
    }

    pub fn report(&self, name: &str) -> Option<&Report> {
        self.reports.get(name)
    }

    /// Applies one action. Returns a run to start when the action asks for one.
    pub fn handle(&mut self, action: Action) -> Option<RunRequest> {
        match self.mode {
            Mode::Filter => self.handle_filter(action),
            Mode::Prompt => return self.handle_prompt(action),
            Mode::Help => {
                if matches!(
                    action,
                    Action::Cancel | Action::Help | Action::Quit | Action::Confirm
                ) {
                    self.mode = Mode::Normal;
                }
            }
            Mode::Normal => return self.handle_normal(action),
        }
        None
    }

    fn handle_normal(&mut self, action: Action) -> Option<RunRequest> {
        match action {
            Action::Up => self.step(-1),
            Action::Down => self.step(1),
            Action::Top => self.jump(true),
            Action::Bottom => self.jump(false),
            Action::NextPanel => self.focus = next(self.focus),
            Action::PrevPanel => self.focus = prev(self.focus),
            Action::Run => return self.request(None),
            Action::StartPrompt => {
                if self.selected().is_some() {
                    self.mode = Mode::Prompt;
                    self.prompt.clear();
                }
            }
            Action::StartFilter => self.mode = Mode::Filter,
            Action::Cancel => {
                self.filter.clear();
                self.message = None;
                self.clamp();
            }
            Action::Help => self.mode = Mode::Help,
            Action::Refresh => self.wants_refresh = true,
            Action::Quit => self.should_quit = true,
            Action::Type(_) | Action::Backspace | Action::Confirm => {}
        }
        None
    }

    fn handle_filter(&mut self, action: Action) {
        match action {
            Action::Type(c) => {
                self.filter.push(c);
                self.cursor = 0;
            }
            Action::Backspace => {
                self.filter.pop();
                self.cursor = 0;
            }
            Action::Confirm => self.mode = Mode::Normal,
            Action::Cancel => {
                self.filter.clear();
                self.mode = Mode::Normal;
                self.cursor = 0;
            }
            _ => {}
        }
    }

    fn handle_prompt(&mut self, action: Action) -> Option<RunRequest> {
        match action {
            Action::Type(c) => self.prompt.push(c),
            Action::Backspace => {
                self.prompt.pop();
            }
            Action::Confirm => {
                self.mode = Mode::Normal;
                let prompt = self.prompt.trim().to_string();
                return self.request(if prompt.is_empty() {
                    None
                } else {
                    Some(prompt)
                });
            }
            Action::Cancel => self.mode = Mode::Normal,
            _ => {}
        }
        None
    }

    fn request(&mut self, prompt: Option<String>) -> Option<RunRequest> {
        let name = self.selected()?.workflow.name.clone();
        if self.running.contains(&name) {
            self.message = Some(format!("{name} is already running"));
            return None;
        }
        self.report_scroll = 0;
        Some((name, prompt))
    }

    fn step(&mut self, delta: isize) {
        if self.focus == Focus::Report {
            self.report_scroll = self.report_scroll.saturating_add_signed(delta);
            return;
        }
        let len = self.visible().len();
        if len == 0 {
            return;
        }
        self.cursor = (self.cursor as isize + delta).clamp(0, len as isize - 1) as usize;
        self.report_scroll = 0;
    }

    fn jump(&mut self, top: bool) {
        if self.focus == Focus::Report {
            self.report_scroll = if top { 0 } else { usize::MAX / 2 };
            return;
        }
        let len = self.visible().len();
        self.cursor = if top || len == 0 { 0 } else { len - 1 };
    }

    fn clamp(&mut self) {
        let len = self.visible().len();
        self.cursor = if len == 0 {
            0
        } else {
            self.cursor.min(len - 1)
        };
    }
}

fn next(focus: Focus) -> Focus {
    match focus {
        Focus::Workflows => Focus::Steps,
        Focus::Steps => Focus::Report,
        Focus::Report => Focus::Workflows,
    }
}

fn prev(focus: Focus) -> Focus {
    match focus {
        Focus::Workflows => Focus::Report,
        Focus::Steps => Focus::Workflows,
        Focus::Report => Focus::Steps,
    }
}
