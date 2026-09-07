//! `hotword ui`: a lazygit-style terminal interface over the same store and
//! runner the CLI uses. `state` is pure and tested; `view` draws it; this file
//! owns the terminal and the run threads.

pub mod state;
pub mod theme;
pub mod view;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::runner::{self, Report, StepResult};
use crate::workflow::{Store, Workflow};
use state::{Action, Mode, State};

enum RunEvent {
    Step(String, StepResult),
    Done(String, Report),
}

pub fn run(store: &Store, base_dir: &Path) -> Result<()> {
    let mut state = State::new(store.load_all()?);
    let (tx, rx) = mpsc::channel::<RunEvent>();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    let result = event_loop(&mut terminal, &mut state, store, base_dir, &tx, &rx);
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut State,
    store: &Store,
    base_dir: &Path,
    tx: &mpsc::Sender<RunEvent>,
    rx: &mpsc::Receiver<RunEvent>,
) -> Result<()> {
    loop {
        terminal.draw(|frame| view::draw(frame, state, store))?;
        while let Ok(ev) = rx.try_recv() {
            match ev {
                RunEvent::Step(name, step) => state.push_step(&name, step),
                RunEvent::Done(name, report) => state.finish(&name, report),
            }
        }
        if state.wants_refresh {
            state.wants_refresh = false;
            match store.load_all() {
                Ok(all) => state.replace_workflows(all),
                Err(err) => state.message = Some(format!("{err:#}")),
            }
        }
        if state.should_quit {
            return Ok(());
        }
        if !event::poll(Duration::from_millis(120))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != event::KeyEventKind::Press {
            continue;
        }
        if state.mode == Mode::Normal && key.code == KeyCode::Char('e') {
            if let Some(path) = state.selected().map(|l| l.path.clone()) {
                edit_in_editor(terminal, &path, state)?;
            }
            continue;
        }
        let Some(action) = to_action(key, state.mode) else {
            continue;
        };
        if let Some((name, prompt)) = state.handle(action) {
            let Some(workflow) = state
                .all
                .iter()
                .find(|l| l.workflow.name == name)
                .map(|l| l.workflow.clone())
            else {
                continue;
            };
            state.mark_running(&name);
            spawn_run(
                workflow,
                base_dir.to_path_buf(),
                prompt.unwrap_or_default(),
                tx.clone(),
            );
        }
    }
}

fn spawn_run(workflow: Workflow, base_dir: PathBuf, prompt: String, tx: mpsc::Sender<RunEvent>) {
    std::thread::spawn(move || {
        let name = workflow.name.clone();
        let step_tx = tx.clone();
        let report = runner::run_streaming(&workflow, &base_dir, &prompt, &mut |step| {
            let _ = step_tx.send(RunEvent::Step(name.clone(), step.clone()));
        });
        let _ = tx.send(RunEvent::Done(name, report));
    });
}

fn edit_in_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    path: &Path,
    state: &mut State,
) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("hotword-edit")
        .arg(path)
        .status()
        .with_context(|| format!("starting {editor}"));
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    match status {
        Ok(s) if s.success() => state.wants_refresh = true,
        Ok(s) => state.message = Some(format!("{editor} exited with {s}")),
        Err(err) => state.message = Some(format!("{err:#}")),
    }
    Ok(())
}

fn to_action(key: KeyEvent, mode: Mode) -> Option<Action> {
    let typing = matches!(mode, Mode::Filter | Mode::Prompt);
    Some(match key.code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => Action::Confirm,
        KeyCode::Backspace if typing => Action::Backspace,
        KeyCode::Char(c) if typing => Action::Type(c),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::Quit,
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('g') | KeyCode::Home => Action::Top,
        KeyCode::Char('G') | KeyCode::End => Action::Bottom,
        KeyCode::Tab | KeyCode::Char('l') | KeyCode::Right => Action::NextPanel,
        KeyCode::BackTab | KeyCode::Char('h') | KeyCode::Left => Action::PrevPanel,
        KeyCode::Char('p') => Action::StartPrompt,
        KeyCode::Char('/') => Action::StartFilter,
        KeyCode::Char('R') => Action::Refresh,
        KeyCode::Char('?') => Action::Help,
        _ => return None,
    })
    .map(|a| {
        if mode == Mode::Normal && a == Action::Confirm {
            Action::Run
        } else {
            a
        }
    })
}
