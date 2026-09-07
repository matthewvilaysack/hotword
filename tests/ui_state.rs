use hotword::runner::{Report, Status, StepResult};
use hotword::ui::state::{Action, Focus, Mode, State};
use hotword::workflow::{Loaded, Source, Workflow};

fn loaded(name: &str, trigger: &str) -> Loaded {
    let wf = Workflow::from_toml(&format!(
        "name = \"{name}\"\ntriggers = [\"{trigger}\"]\n[[steps]]\nname = \"a\"\nrun = \"echo a\"\n[[steps]]\nname = \"b\"\nrun = \"echo b\"\n"
    ))
    .unwrap();
    Loaded {
        workflow: wf,
        path: format!("/tmp/{name}.toml").into(),
        source: Source::User,
    }
}

fn state() -> State {
    State::new(vec![
        loaded("apple-status", "apple status"),
        loaded("pr-review", "review pr"),
        loaded("repo-status", "repo status"),
    ])
}

#[test]
fn selection_moves_and_clamps() {
    let mut s = state();
    assert_eq!(s.selected().unwrap().workflow.name, "apple-status");
    s.handle(Action::Down);
    s.handle(Action::Down);
    s.handle(Action::Down);
    assert_eq!(s.selected().unwrap().workflow.name, "repo-status");
    s.handle(Action::Up);
    assert_eq!(s.selected().unwrap().workflow.name, "pr-review");
    s.handle(Action::Top);
    assert_eq!(s.selected().unwrap().workflow.name, "apple-status");
    s.handle(Action::Bottom);
    assert_eq!(s.selected().unwrap().workflow.name, "repo-status");
}

#[test]
fn filter_narrows_the_list_by_name_or_trigger() {
    let mut s = state();
    s.handle(Action::StartFilter);
    assert_eq!(s.mode, Mode::Filter);
    for c in "review".chars() {
        s.handle(Action::Type(c));
    }
    let names: Vec<_> = s
        .visible()
        .iter()
        .map(|l| l.workflow.name.as_str())
        .collect();
    assert_eq!(names, vec!["pr-review"]);
    s.handle(Action::Confirm);
    assert_eq!(s.mode, Mode::Normal);
    assert_eq!(s.selected().unwrap().workflow.name, "pr-review");
    s.handle(Action::Cancel);
    assert_eq!(s.visible().len(), 3);
}

#[test]
fn prompt_mode_collects_text_and_requests_a_run_with_it() {
    let mut s = state();
    s.handle(Action::Down);
    s.handle(Action::StartPrompt);
    for c in "review pr 42".chars() {
        s.handle(Action::Type(c));
    }
    s.handle(Action::Backspace);
    s.handle(Action::Type('2'));
    let request = s.handle(Action::Confirm);
    assert_eq!(
        request,
        Some(("pr-review".to_string(), Some("review pr 42".to_string())))
    );
    assert_eq!(s.mode, Mode::Normal);
}

#[test]
fn run_requests_are_refused_while_that_workflow_is_running() {
    let mut s = state();
    assert_eq!(
        s.handle(Action::Run),
        Some(("apple-status".to_string(), None))
    );
    s.mark_running("apple-status");
    assert_eq!(s.handle(Action::Run), None);
    assert!(s.is_running("apple-status"));
}

#[test]
fn step_results_stream_in_and_finish_into_a_report() {
    let mut s = state();
    s.mark_running("apple-status");
    s.push_step(
        "apple-status",
        StepResult {
            name: "a".into(),
            status: Status::Ok,
            exit: Some(0),
            ms: 5,
            output: "a\n".into(),
            reason: None,
        },
    );
    assert_eq!(s.progress("apple-status"), Some((1, 2)));
    let report = Report {
        workflow: "apple-status".into(),
        description: None,
        max_lines: 40,
        steps: vec![
            StepResult {
                name: "a".into(),
                status: Status::Ok,
                exit: Some(0),
                ms: 5,
                output: "a\n".into(),
                reason: None,
            },
            StepResult {
                name: "b".into(),
                status: Status::Fail,
                exit: Some(1),
                ms: 7,
                output: "boom\n".into(),
                reason: None,
            },
        ],
    };
    s.finish("apple-status", report);
    assert!(!s.is_running("apple-status"));
    assert_eq!(s.report("apple-status").unwrap().count(Status::Fail), 1);
    assert_eq!(s.progress("apple-status"), None);
}

#[test]
fn focus_cycles_between_panels_and_help_toggles() {
    let mut s = state();
    assert_eq!(s.focus, Focus::Workflows);
    s.handle(Action::NextPanel);
    assert_eq!(s.focus, Focus::Steps);
    s.handle(Action::NextPanel);
    assert_eq!(s.focus, Focus::Report);
    s.handle(Action::NextPanel);
    assert_eq!(s.focus, Focus::Workflows);
    s.handle(Action::PrevPanel);
    assert_eq!(s.focus, Focus::Report);
    s.handle(Action::Help);
    assert_eq!(s.mode, Mode::Help);
    s.handle(Action::Cancel);
    assert_eq!(s.mode, Mode::Normal);
}

#[test]
fn report_scrolls_only_when_the_report_panel_is_focused() {
    let mut s = state();
    s.handle(Action::NextPanel);
    s.handle(Action::NextPanel);
    s.handle(Action::Down);
    s.handle(Action::Down);
    assert_eq!(s.report_scroll, 2);
    s.handle(Action::Up);
    assert_eq!(s.report_scroll, 1);
    assert_eq!(s.selected().unwrap().workflow.name, "apple-status");
}

#[test]
fn quit_is_only_a_quit_in_normal_mode() {
    let mut s = state();
    s.handle(Action::StartFilter);
    s.handle(Action::Type('q'));
    assert!(!s.should_quit);
    s.handle(Action::Cancel);
    s.handle(Action::Quit);
    assert!(s.should_quit);
}
