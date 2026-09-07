use std::time::Instant;

use hotword::runner::{run, Status};
use hotword::workflow::Workflow;

fn wf(steps: &str) -> Workflow {
    Workflow::from_toml(&format!("name = \"t\"\ntriggers = [\"t\"]\n{steps}")).unwrap()
}

#[test]
fn ok_step_captures_stdout() {
    let report = run(
        &wf("[[steps]]\nname = \"hi\"\nrun = \"echo hi\"\n"),
        std::env::temp_dir().as_path(),
    );
    assert_eq!(report.workflow, "t");
    let step = &report.steps[0];
    assert_eq!(step.status, Status::Ok);
    assert_eq!(step.exit, Some(0));
    assert_eq!(step.output.trim(), "hi");
}

#[test]
fn failing_step_reports_exit_code_and_stderr() {
    let report = run(
        &wf("[[steps]]\nname = \"boom\"\nrun = \"echo oops >&2; exit 3\"\n"),
        std::env::temp_dir().as_path(),
    );
    let step = &report.steps[0];
    assert_eq!(step.status, Status::Fail);
    assert_eq!(step.exit, Some(3));
    assert!(step.output.contains("oops"));
}

#[test]
fn missing_binary_is_skipped_with_a_reason() {
    let report = run(
        &wf("[[steps]]\nname = \"m\"\nrun = \"definitely-not-a-binary-xyz --flag\"\n"),
        std::env::temp_dir().as_path(),
    );
    let step = &report.steps[0];
    assert_eq!(step.status, Status::Skip);
    assert_eq!(
        step.reason.as_deref(),
        Some("definitely-not-a-binary-xyz not on PATH")
    );
}

#[test]
fn shell_builtin_first_word_still_runs() {
    let report = run(
        &wf("[[steps]]\nname = \"b\"\nrun = \"cd . && echo built\"\n"),
        std::env::temp_dir().as_path(),
    );
    assert_eq!(report.steps[0].status, Status::Ok);
    assert_eq!(report.steps[0].output.trim(), "built");
}

#[test]
fn explicit_requires_overrides_the_first_word() {
    let report = run(
        &wf("[[steps]]\nname = \"r\"\nrun = \"echo never\"\nrequires = [\"definitely-not-a-binary-xyz\"]\n"),
        std::env::temp_dir().as_path(),
    );
    assert_eq!(report.steps[0].status, Status::Skip);
}

#[test]
fn step_timeout_kills_the_command() {
    let started = Instant::now();
    let report = run(
        &wf("[[steps]]\nname = \"slow\"\nrun = \"sleep 5\"\ntimeout = 1\n"),
        std::env::temp_dir().as_path(),
    );
    assert_eq!(report.steps[0].status, Status::Timeout);
    assert!(started.elapsed().as_secs() < 4);
    assert_eq!(
        report.steps[0].reason.as_deref(),
        Some("timed out after 1s")
    );
}

#[test]
fn step_cwd_is_honoured_and_tilde_expands() {
    let tmp = tempfile::tempdir().unwrap();
    let report = run(
        &wf(&format!(
            "[[steps]]\nname = \"where\"\nrun = \"pwd\"\ncwd = \"{}\"\n",
            tmp.path().display()
        )),
        std::env::temp_dir().as_path(),
    );
    let got = std::fs::canonicalize(report.steps[0].output.trim()).unwrap();
    assert_eq!(got, std::fs::canonicalize(tmp.path()).unwrap());

    let home = std::env::var("HOME").unwrap();
    let report = run(
        &wf("[[steps]]\nname = \"home\"\nrun = \"pwd\"\ncwd = \"~\"\n"),
        std::env::temp_dir().as_path(),
    );
    assert_eq!(
        std::fs::canonicalize(report.steps[0].output.trim()).unwrap(),
        std::fs::canonicalize(home).unwrap()
    );
}

#[test]
fn steps_run_in_order_and_all_run_even_after_a_failure() {
    let report = run(
        &wf("[[steps]]\nname = \"a\"\nrun = \"exit 1\"\n[[steps]]\nname = \"b\"\nrun = \"echo b\"\n"),
        std::env::temp_dir().as_path(),
    );
    let statuses: Vec<_> = report.steps.iter().map(|s| s.status).collect();
    assert_eq!(statuses, vec![Status::Fail, Status::Ok]);
}

#[test]
fn missing_cwd_skips_the_step_with_a_reason() {
    let report = run(
        &wf("[[steps]]\nname = \"gone\"\nrun = \"pwd\"\ncwd = \"/definitely/not/here\"\n"),
        std::env::temp_dir().as_path(),
    );
    assert_eq!(report.steps[0].status, Status::Skip);
    assert_eq!(
        report.steps[0].reason.as_deref(),
        Some("/definitely/not/here does not exist")
    );
}

#[test]
fn steps_see_the_prompt_and_matched_trigger_in_the_environment() {
    use hotword::runner::run_for_prompt;
    let workflow =
        wf("[[steps]]\nname = \"env\"\nrun = \"echo \\\"$HOTWORD_TRIGGER|$HOTWORD_PROMPT\\\"\"\n");
    let report = run_for_prompt(
        &workflow,
        std::env::temp_dir().as_path(),
        "please T the thing #12",
    );
    assert_eq!(report.steps[0].output.trim(), "t|please T the thing #12");

    let report = run(&workflow, std::env::temp_dir().as_path());
    assert_eq!(report.steps[0].output.trim(), "|");
}

#[test]
fn streaming_reports_each_step_as_it_finishes() {
    use hotword::runner::run_streaming;
    let workflow = wf("[[steps]]\nname = \"one\"\nrun = \"echo 1\"\n[[steps]]\nname = \"two\"\nrun = \"echo 2\"\n");
    let mut seen = Vec::new();
    let report = run_streaming(&workflow, std::env::temp_dir().as_path(), "", &mut |s| {
        seen.push(s.name.clone())
    });
    assert_eq!(seen, vec!["one", "two"]);
    assert_eq!(report.steps.len(), 2);
}
