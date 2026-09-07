//! Turns a run into guidance. Each step gets a verdict, the cause in plain
//! words, and the fixes to try, so a person can repair a broken workflow
//! without reading shell output. Deterministic: every rule is a string match.

use serde::Serialize;

use crate::runner::{Report, Status, StepResult};
use crate::workflow::Workflow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Healthy,
    MissingTool,
    WrongFolder,
    NeedsAuth,
    Network,
    Permission,
    SilentFailure,
    TooSlow,
    Failed,
    Skipped,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Healthy => "healthy",
            Verdict::MissingTool => "missing-tool",
            Verdict::WrongFolder => "wrong-folder",
            Verdict::NeedsAuth => "needs-auth",
            Verdict::Network => "network",
            Verdict::Permission => "permission",
            Verdict::SilentFailure => "silent-failure",
            Verdict::TooSlow => "too-slow",
            Verdict::Failed => "failed",
            Verdict::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StepDiagnosis {
    pub name: String,
    pub run: String,
    pub status: Status,
    pub verdict: Verdict,
    pub cause: String,
    pub fixes: Vec<String>,
    pub output: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnosis {
    pub workflow: String,
    pub problems: usize,
    pub total: usize,
    pub first_problem: Option<String>,
    pub steps: Vec<StepDiagnosis>,
}

pub fn diagnose(workflow: &Workflow, report: &Report) -> Diagnosis {
    let steps: Vec<StepDiagnosis> = report
        .steps
        .iter()
        .map(|result| {
            let step = workflow.steps.iter().find(|s| s.name == result.name);
            let run = step.map(|s| s.run.clone()).unwrap_or_default();
            let (verdict, cause, fixes) = judge(result, &run, step.and_then(|s| s.cwd.clone()));
            StepDiagnosis {
                name: result.name.clone(),
                run,
                status: result.status,
                verdict,
                cause,
                fixes,
                output: result.output.clone(),
            }
        })
        .collect();
    let problems = steps
        .iter()
        .filter(|s| s.verdict != Verdict::Healthy)
        .count();
    let first_problem = steps
        .iter()
        .find(|s| s.verdict != Verdict::Healthy)
        .map(|s| s.name.clone());
    Diagnosis {
        workflow: workflow.name.clone(),
        problems,
        total: steps.len(),
        first_problem,
        steps,
    }
}

fn judge(result: &StepResult, run: &str, cwd: Option<String>) -> (Verdict, String, Vec<String>) {
    let text = result.output.to_lowercase();
    let reason = result.reason.clone().unwrap_or_default();
    let first = run
        .split_whitespace()
        .next()
        .unwrap_or("the command")
        .to_string();
    match result.status {
        Status::Ok => (Verdict::Healthy, "Ran and exited 0.".into(), Vec::new()),
        Status::Skip if reason.ends_with("not on PATH") => {
            let tool = reason.trim_end_matches(" not on PATH").to_string();
            (
                Verdict::MissingTool,
                format!("{tool} is not installed here, or not on the PATH the agent's hooks see."),
                vec![
                    format!("Install {tool}, or add its folder to PATH in your shell profile."),
                    format!("If {tool} is not really the binary this step needs, set requires = [\"<binary>\"] on the step; requires = [] runs it regardless."),
                    "Skipped steps do not fail the workflow; leave it if this machine is simply not meant to have the tool.".into(),
                ],
            )
        }
        Status::Skip if reason.ends_with("does not exist") => (
            Verdict::WrongFolder,
            format!("The step's cwd points at a folder this machine does not have: {}.", cwd.unwrap_or_default()),
            vec![
                "Change cwd to a folder that exists here, or remove it so the step runs where the agent is.".into(),
                "Keep it if the folder is only on another machine; the step will run there and skip here.".into(),
            ],
        ),
        Status::Skip => (Verdict::Skipped, reason, vec!["Check the step's requires and cwd fields.".into()]),
        Status::Timeout => (
            Verdict::TooSlow,
            format!("{}; the whole process group was killed.", reason.trim_end_matches('.')),
            vec![
                "Raise timeout on the step (or the workflow) if the command legitimately takes that long.".into(),
                "Narrow the command: add --limit, head, or a smaller date range so it finishes fast.".into(),
                "If it waits for input, it will never finish under a hook; give it every flag it needs or pipe from /dev/null.".into(),
            ],
        ),
        Status::Fail => {
            if text.contains("command not found") || text.contains("no such file or directory") && text.contains(&first.to_lowercase()) {
                (
                    Verdict::MissingTool,
                    format!("The shell could not find a command this step calls ({first} or something it runs)."),
                    vec![
                        format!("Check the spelling of {first} and that it is installed on this machine."),
                        format!("If it is installed, hooks may see a shorter PATH: use the full path, or list it in requires = [\"{first}\"] so the step skips cleanly instead."),
                    ],
                )
            } else if text.contains("not a git repository") || text.contains("no such file or directory") {
                (
                    Verdict::WrongFolder,
                    "The command ran in a folder that does not have what it expects.".into(),
                    vec![
                        "Set cwd on the step to the repository or folder it needs (~ is allowed).".into(),
                        "Or open the agent in that folder; hook runs use the agent's current directory.".into(),
                    ],
                )
            } else if text.contains("auth login") || text.contains("401") || text.contains("unauthorized") || text.contains("bad credentials") || text.contains("not logged in") || text.contains("authentication") {
                (
                    Verdict::NeedsAuth,
                    "The tool wants credentials it does not have in the hook's environment.".into(),
                    vec![
                        if first == "gh" || text.contains("gh auth") { "Run `gh auth login` in a terminal once; hooks reuse that login.".into() } else { format!("Log in with {first}'s own login command in a terminal once.") },
                        "Hooks do not get a TTY, so a login prompt cannot open there; do it in a normal terminal.".into(),
                    ],
                )
            } else if text.contains("error connecting") || text.contains("could not resolve") || text.contains("connection refused") || text.contains("timed out") || text.contains("network is unreachable") || text.contains("check your internet") {
                (
                    Verdict::Network,
                    "The command could not reach its host.".into(),
                    vec![
                        "Check VPN or the network; internal hosts often need the corporate VPN.".into(),
                        "Give the step a short timeout so a dead network fails fast, and let it fail: the rest of the workflow still runs.".into(),
                    ],
                )
            } else if text.contains("permission denied") || text.contains("operation not permitted") {
                (
                    Verdict::Permission,
                    "The command was refused by file or system permissions.".into(),
                    vec![
                        "Check the file is executable (chmod +x) and readable by your user.".into(),
                        "Hooks run as you, without sudo; a command that needs elevated rights cannot run here.".into(),
                    ],
                )
            } else if result.output.trim().is_empty() {
                (
                    Verdict::SilentFailure,
                    format!("Exited {} with no output; grep, diff, and test exit 1 when they simply find nothing.", result.exit.unwrap_or(1)),
                    vec![
                        "If no match is a fine answer, append `|| true` so the step reads ok.".into(),
                        "If it should have found something, run the command in a terminal to see why it did not.".into(),
                    ],
                )
            } else {
                (
                    Verdict::Failed,
                    format!("Exited {}; the last line of output usually says why.", result.exit.unwrap_or(1)),
                    vec![
                        format!("Run it by hand: `hotword doctor {{workflow}} --step {}` shows the full output.", result.name),
                        "Fix the command, then run the workflow again; steps after it already ran, so nothing else waits on it.".into(),
                    ],
                )
            }
        }
    }
}

/// The text form: a summary table, then cause and fixes per problem step.
pub fn render_text(d: &Diagnosis) -> String {
    let mut out = format!("doctor: {}\n", d.workflow);
    out.push_str(&format!("problems: {} of {} steps\n", d.problems, d.total));
    if let Some(first) = &d.first_problem {
        out.push_str(&format!("start with: {first}\n"));
    }
    out.push_str(&format!(
        "steps[{}]{{name,status,verdict}}:\n",
        d.steps.len()
    ));
    for s in &d.steps {
        out.push_str(&format!(
            "  {},{},{}\n",
            s.name,
            s.status.as_str(),
            s.verdict.as_str()
        ));
    }
    for s in d.steps.iter().filter(|s| s.verdict != Verdict::Healthy) {
        out.push_str(&format!("\n{} ({}):\n", s.name, s.verdict.as_str()));
        out.push_str(&format!("  run: {}\n", s.run));
        out.push_str(&format!("  cause: {}\n", s.cause));
        for fix in &s.fixes {
            out.push_str(&format!(
                "  fix: {}\n",
                fix.replace("{workflow}", &d.workflow)
            ));
        }
        let tail: Vec<&str> = s.output.lines().rev().take(6).collect();
        if !tail.is_empty() {
            out.push_str("  output:\n");
            for line in tail.iter().rev() {
                out.push_str(&format!("    {line}\n"));
            }
        }
    }
    if d.problems == 0 {
        out.push_str("\nEvery step is healthy.\n");
    }
    out
}
