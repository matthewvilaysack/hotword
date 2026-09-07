//! Renders a run as compact text for a human or an agent, as JSON, and as the
//! hook envelope Claude Code and Codex read from stdout.

use serde_json::json;

use crate::runner::{Report, Status};

/// Both Claude Code and Codex cut additionalContext around 10,000 characters
/// and swap in a file preview. Stay under that so the report itself arrives.
pub const CONTEXT_CAP: usize = 9_000;

const CAP_NOTE: &str = "(output capped; run the workflow directly for the rest)";

pub fn render_text(report: &Report, full: bool) -> String {
    let mut out = format!("hotword: {}\n", report.workflow);
    if let Some(description) = &report.description {
        out.push_str(&format!("description: {description}\n"));
    }
    out.push_str(&format!(
        "steps[{}]{{name,status,exit,ms}}:\n",
        report.steps.len()
    ));
    for step in &report.steps {
        let exit = step.exit.map_or("-".to_string(), |c| c.to_string());
        out.push_str(&format!(
            "  {},{},{},{}\n",
            step.name,
            step.status.as_str(),
            exit,
            step.ms
        ));
    }

    let mut truncated_any = false;
    for step in &report.steps {
        out.push('\n');
        match step.status {
            Status::Skip => {
                out.push_str(&format!(
                    "{}: skipped, {}\n",
                    step.name,
                    step.reason.as_deref().unwrap_or("")
                ));
                continue;
            }
            Status::Timeout => out.push_str(&format!(
                "{} ({}):\n",
                step.name,
                step.reason.as_deref().unwrap_or("timed out")
            )),
            Status::Fail => out.push_str(&format!(
                "{} (exit {}):\n",
                step.name,
                step.exit.map_or("-".to_string(), |c| c.to_string())
            )),
            Status::Ok => out.push_str(&format!("{}:\n", step.name)),
        }
        if let Some(reason) = step
            .reason
            .as_deref()
            .filter(|_| step.status == Status::Fail)
        {
            out.push_str(&format!("  {reason}\n"));
        }
        let lines: Vec<&str> = step.output.lines().collect();
        if lines.is_empty() {
            out.push_str("  (no output)\n");
            continue;
        }
        let shown = if full || lines.len() <= report.max_lines {
            lines.len()
        } else {
            report.max_lines
        };
        for line in &lines[..shown] {
            out.push_str(&format!("  {line}\n"));
        }
        if shown < lines.len() {
            truncated_any = true;
            out.push_str(&format!("  ... (truncated, {} lines total)\n", lines.len()));
        }
    }
    if truncated_any {
        out.push_str(&format!(
            "\nhelp: run `hotword run {} --full` for untruncated output\n",
            report.workflow
        ));
    }
    out
}

pub fn render_json(report: &Report) -> String {
    let value = json!({
        "workflow": report.workflow,
        "description": report.description,
        "summary": {
            "ok": report.count(Status::Ok),
            "fail": report.count(Status::Fail),
            "skip": report.count(Status::Skip),
            "timeout": report.count(Status::Timeout),
        },
        "steps": report.steps,
    });
    serde_json::to_string_pretty(&value).expect("report serializes")
}

/// The stdout envelope for a SessionStart or UserPromptSubmit command hook.
pub fn hook_json(event_name: &str, context: &str) -> String {
    let context = cap(context);
    let value = json!({
        "hookSpecificOutput": {
            "hookEventName": event_name,
            "additionalContext": context,
        }
    });
    serde_json::to_string(&value).expect("hook output serializes")
}

fn cap(context: &str) -> String {
    if context.len() <= CONTEXT_CAP {
        return context.to_string();
    }
    let mut end = CONTEXT_CAP;
    while !context.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n{CAP_NOTE}", &context[..end])
}
