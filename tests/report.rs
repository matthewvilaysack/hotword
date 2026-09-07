use hotword::report::{hook_json, render_json, render_text, CONTEXT_CAP};
use hotword::runner::{Report, Status, StepResult};

fn report() -> Report {
    Report {
        workflow: "deploy-status".into(),
        description: Some("Where things stand".into()),
        max_lines: 2,
        steps: vec![
            StepResult {
                name: "version".into(),
                status: Status::Ok,
                exit: Some(0),
                ms: 12,
                output: "0.2.68\n".into(),
                reason: None,
            },
            StepResult {
                name: "doctor".into(),
                status: Status::Fail,
                exit: Some(1),
                ms: 340,
                output: "l1\nl2\nl3\nl4\n".into(),
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
fn text_report_has_toon_summary_then_outputs() {
    let text = render_text(&report(), false);
    assert!(text.starts_with("hotword: deploy-status\n"), "{text}");
    assert!(text.contains("steps[3]{name,status,exit,ms}:\n  version,ok,0,12\n  doctor,fail,1,340\n  prs,skip,-,0\n"), "{text}");
    assert!(text.contains("version:\n  0.2.68\n"), "{text}");
    assert!(text.contains("prs: skipped, gh not on PATH\n"), "{text}");
}

#[test]
fn text_report_truncates_long_output_and_says_how_to_get_it_all() {
    let text = render_text(&report(), false);
    assert!(
        text.contains("doctor (exit 1):\n  l1\n  l2\n  ... (truncated, 4 lines total)\n"),
        "{text}"
    );
    assert!(
        text.contains("help: run `hotword run deploy-status --full` for untruncated output"),
        "{text}"
    );
    let full = render_text(&report(), true);
    assert!(full.contains("  l3\n  l4\n"), "{full}");
    assert!(!full.contains("truncated"), "{full}");
}

#[test]
fn json_report_is_machine_readable() {
    let value: serde_json::Value = serde_json::from_str(&render_json(&report())).unwrap();
    assert_eq!(value["workflow"], "deploy-status");
    assert_eq!(value["summary"]["ok"], 1);
    assert_eq!(value["summary"]["fail"], 1);
    assert_eq!(value["summary"]["skip"], 1);
    assert_eq!(value["steps"][1]["exit"], 1);
    assert_eq!(value["steps"][2]["reason"], "gh not on PATH");
}

#[test]
fn hook_json_wraps_context_for_the_event() {
    let value: serde_json::Value =
        serde_json::from_str(&hook_json("UserPromptSubmit", "ctx")).unwrap();
    assert_eq!(
        value["hookSpecificOutput"]["hookEventName"],
        "UserPromptSubmit"
    );
    assert_eq!(value["hookSpecificOutput"]["additionalContext"], "ctx");
}

#[test]
fn hook_json_caps_context_under_the_agent_limit() {
    let huge = "x".repeat(CONTEXT_CAP * 2);
    let value: serde_json::Value = serde_json::from_str(&hook_json("SessionStart", &huge)).unwrap();
    let ctx = value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.len() <= CONTEXT_CAP + 200, "{}", ctx.len());
    assert!(ctx.ends_with("(output capped; run the workflow directly for the rest)"));
}
