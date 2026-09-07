use hotword::doctor::{diagnose, Verdict};
use hotword::runner::{Report, Status, StepResult};
use hotword::workflow::Workflow;

fn wf() -> Workflow {
    Workflow::from_toml(
        "name = \"t\"\ntriggers = [\"t\"]\n[[steps]]\nname = \"a\"\nrun = \"maps-cli --version\"\n[[steps]]\nname = \"b\"\nrun = \"gh pr list\"\n[[steps]]\nname = \"c\"\nrun = \"grep needle file.txt\"\n[[steps]]\nname = \"d\"\nrun = \"sleep 90\"\n[[steps]]\nname = \"e\"\nrun = \"echo fine\"\n",
    )
    .unwrap()
}

fn step(
    name: &str,
    status: Status,
    exit: Option<i32>,
    output: &str,
    reason: Option<&str>,
) -> StepResult {
    StepResult {
        name: name.into(),
        status,
        exit,
        ms: 1,
        output: output.into(),
        reason: reason.map(String::from),
    }
}

#[test]
fn every_step_gets_a_verdict_with_a_cause_and_a_fix() {
    let report = Report {
        workflow: "t".into(),
        description: None,
        max_lines: 40,
        steps: vec![
            step("a", Status::Skip, None, "", Some("maps-cli not on PATH")),
            step(
                "b",
                Status::Fail,
                Some(4),
                "gh: To get started with GitHub CLI, please run:  gh auth login\n",
                None,
            ),
            step("c", Status::Fail, Some(1), "", None),
            step("d", Status::Timeout, None, "", Some("timed out after 30s")),
            step("e", Status::Ok, Some(0), "fine\n", None),
        ],
    };
    let d = diagnose(&wf(), &report);
    assert_eq!(d.steps.len(), 5);
    assert_eq!(d.steps[0].verdict, Verdict::MissingTool);
    assert!(
        d.steps[0].cause.contains("maps-cli"),
        "{}",
        d.steps[0].cause
    );
    assert!(
        d.steps[0].fixes.iter().any(|f| f.contains("requires")),
        "{:?}",
        d.steps[0].fixes
    );
    assert_eq!(d.steps[1].verdict, Verdict::NeedsAuth);
    assert!(
        d.steps[1].fixes.iter().any(|f| f.contains("gh auth login")),
        "{:?}",
        d.steps[1].fixes
    );
    assert_eq!(d.steps[2].verdict, Verdict::SilentFailure);
    assert!(
        d.steps[2].fixes.iter().any(|f| f.contains("|| true")),
        "{:?}",
        d.steps[2].fixes
    );
    assert_eq!(d.steps[3].verdict, Verdict::TooSlow);
    assert!(
        d.steps[3].fixes.iter().any(|f| f.contains("timeout")),
        "{:?}",
        d.steps[3].fixes
    );
    assert_eq!(d.steps[4].verdict, Verdict::Healthy);
    assert_eq!(d.first_problem.as_deref(), Some("a"));
    assert_eq!(d.problems, 4);
}

#[test]
fn common_shell_failures_are_named() {
    let cases = [
        ("sh: foo: command not found\n", Verdict::MissingTool),
        (
            "fatal: not a git repository (or any of the parent directories): .git\n",
            Verdict::WrongFolder,
        ),
        ("bash: ./x: Permission denied\n", Verdict::Permission),
        (
            "error connecting to github.geo.apple.com\ncheck your internet connection\n",
            Verdict::Network,
        ),
        ("Error: HTTP 401: Bad credentials\n", Verdict::NeedsAuth),
        ("something else broke\n", Verdict::Failed),
    ];
    for (output, expected) in cases {
        let report = Report {
            workflow: "t".into(),
            description: None,
            max_lines: 40,
            steps: vec![step("b", Status::Fail, Some(1), output, None)],
        };
        let d = diagnose(&wf(), &report);
        assert_eq!(d.steps[0].verdict, expected, "for output {output:?}");
        assert!(!d.steps[0].fixes.is_empty());
    }
}

#[test]
fn missing_folder_skip_points_at_cwd() {
    let report = Report {
        workflow: "t".into(),
        description: None,
        max_lines: 40,
        steps: vec![step(
            "a",
            Status::Skip,
            None,
            "",
            Some("/Users/x/code/nucleus does not exist"),
        )],
    };
    let d = diagnose(&wf(), &report);
    assert_eq!(d.steps[0].verdict, Verdict::WrongFolder);
    assert!(d.steps[0].fixes.iter().any(|f| f.contains("cwd")));
}
