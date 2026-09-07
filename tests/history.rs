//! These drive a real Dolt repository in a temp directory. They are skipped
//! with a note when `dolt` is not on PATH, so `make test` stays honest on a
//! machine without it.


use hotword::history::History;
use hotword::runner::{Report, Status, StepResult};

fn dolt_missing() -> bool {
    if hotword::runner::on_path("dolt") {
        return false;
    }
    eprintln!("dolt not on PATH, skipping history test");
    true
}

fn sandbox() -> (tempfile::TempDir, History) {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("home")).unwrap();
    let history = History::new(tmp.path().join("hotword/history"), tmp.path().join("home"));
    (tmp, history)
}

fn report(workflow: &str, first_output: &str) -> Report {
    Report {
        workflow: workflow.into(),
        description: None,
        max_lines: 40,
        steps: vec![
            StepResult {
                name: "branch".into(),
                status: Status::Ok,
                exit: Some(0),
                ms: 12,
                output: first_output.into(),
                reason: None,
            },
            StepResult {
                name: "prs".into(),
                status: Status::Skip,
                exit: None,
                ms: 0,
                output: String::new(),
                reason: Some("gh not on PATH".into()),
            },
        ],
    }
}

#[test]
fn history_is_absent_until_initialised_and_recording_is_then_a_no_op() {
    if dolt_missing() {
        return;
    }
    let (_tmp, history) = sandbox();
    assert!(!history.exists());
    assert!(history.record(&report("repo-status", "main"), None).is_ok());
    assert!(!history.exists());
}

#[test]
fn init_creates_a_dolt_repo_with_the_tables() {
    if dolt_missing() {
        return;
    }
    let (_tmp, history) = sandbox();
    history.init().unwrap();
    assert!(history.exists());
    assert!(history.dir.join(".dolt").is_dir());
    let tables = history.sql_json("SHOW TABLES").unwrap();
    let names: Vec<String> = tables
        .iter()
        .filter_map(|r| r.values().next().and_then(|v| v.as_str()).map(String::from))
        .collect();
    assert!(
        names.contains(&"runs".to_string())
            && names.contains(&"steps".to_string())
            && names.contains(&"latest".to_string()),
        "{names:?}"
    );
    history.init().unwrap();
}

#[test]
fn each_run_becomes_a_commit_and_the_latest_table_diffs_between_runs() {
    if dolt_missing() {
        return;
    }
    let (_tmp, history) = sandbox();
    history.init().unwrap();
    history
        .record(&report("repo-status", "## main\n"), Some("repo status"))
        .unwrap();
    history
        .record(&report("repo-status", "## main\n M src/lib.rs\n"), None)
        .unwrap();

    let runs = history.runs(Some("repo-status"), 10).unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].workflow, "repo-status");
    assert_eq!(runs[0].ok, 1);
    assert_eq!(runs[0].skip, 1);
    assert_eq!(runs[1].prompt.as_deref(), Some("repo status"));

    let changes = history.changes("repo-status").unwrap();
    let branch = changes
        .iter()
        .find(|c| c.name == "branch")
        .expect("branch step changed");
    assert!(branch.output_changed);
    assert!(branch.after.contains("M src/lib.rs"));
    assert!(
        changes.iter().all(|c| c.name != "prs"),
        "unchanged steps are not listed: {changes:?}"
    );

    let log = history
        .sql_json("SELECT COUNT(*) AS n FROM dolt_log")
        .unwrap();
    assert_eq!(
        log[0]["n"]
            .as_i64()
            .or_else(|| log[0]["n"].as_str().and_then(|s| s.parse().ok())),
        Some(4)
    );
}
