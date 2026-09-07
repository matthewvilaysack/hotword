//! Runs a workflow's steps in order through `sh`, one at a time, each with its
//! own timeout. Steps whose binary is not on PATH are skipped rather than failed
//! so a shared workflow degrades gracefully on a machine without the tool.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use wait_timeout::ChildExt;

use crate::workflow::{Step, Workflow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Ok,
    Fail,
    Skip,
    Timeout,
}

impl Status {
    pub fn as_str(&self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Fail => "fail",
            Status::Skip => "skip",
            Status::Timeout => "timeout",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub name: String,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    pub ms: u128,
    pub output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub workflow: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub max_lines: usize,
    pub steps: Vec<StepResult>,
}

impl Report {
    pub fn count(&self, status: Status) -> usize {
        self.steps.iter().filter(|s| s.status == status).count()
    }
}

/// Words that `sh` handles itself, so their absence from PATH means nothing.
const BUILTINS: &[&str] = &[
    "cd", "echo", "export", "for", "if", "while", "until", "case", "set", "source", ".", "test",
    "[", "[[", "true", "false", "printf", "pwd", "read", "eval", "exec", "time", "command", "env",
    "exit", "return", "unset", "shift", "trap", "wait", "umask", "ulimit", "type", "hash", "alias",
    "local", "break", "continue", "{", "(", "!",
];

pub fn run(workflow: &Workflow, base_dir: &Path) -> Report {
    run_inner(workflow, base_dir, "", "")
}

/// Like `run`, but steps also see the prompt that fired the workflow as
/// `HOTWORD_PROMPT` and the matching phrase as `HOTWORD_TRIGGER`, so a step
/// can pull a PR number or a branch name out of what the person typed.
pub fn run_for_prompt(workflow: &Workflow, base_dir: &Path, prompt: &str) -> Report {
    let trigger = workflow
        .triggers
        .iter()
        .find(|t| workflow.matches_trigger(t, prompt))
        .cloned()
        .unwrap_or_default();
    run_inner(workflow, base_dir, prompt, &trigger)
}

fn run_inner(workflow: &Workflow, base_dir: &Path, prompt: &str, trigger: &str) -> Report {
    let steps = workflow
        .steps
        .iter()
        .map(|step| run_step(step, workflow.timeout, base_dir, prompt, trigger))
        .collect();
    Report {
        workflow: workflow.name.clone(),
        description: workflow.description.clone(),
        max_lines: workflow.max_lines,
        steps,
    }
}

fn run_step(
    step: &Step,
    default_timeout: u64,
    base_dir: &Path,
    prompt: &str,
    trigger: &str,
) -> StepResult {
    let result = |status, exit, ms, output, reason: Option<String>| StepResult {
        name: step.name.clone(),
        status,
        exit,
        ms,
        output,
        reason,
    };

    if let Some(missing) = required_binaries(step).into_iter().find(|b| !on_path(b)) {
        return result(
            Status::Skip,
            None,
            0,
            String::new(),
            Some(format!("{missing} not on PATH")),
        );
    }

    let timeout = step.timeout.unwrap_or(default_timeout);
    let cwd = step
        .cwd
        .as_deref()
        .map(expand_tilde)
        .unwrap_or_else(|| base_dir.to_path_buf());
    if !cwd.is_dir() {
        return result(
            Status::Skip,
            None,
            0,
            String::new(),
            Some(format!("{} does not exist", cwd.display())),
        );
    }
    let started = Instant::now();
    let spawned = Command::new("sh")
        .arg("-c")
        .arg(format!("{{ {}\n}} 2>&1", step.run))
        .current_dir(&cwd)
        .env("HOTWORD_PROMPT", prompt)
        .env("HOTWORD_TRIGGER", trigger)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(err) => {
            return result(
                Status::Fail,
                None,
                0,
                String::new(),
                Some(format!("could not start sh: {err}")),
            )
        }
    };

    let mut stdout = child.stdout.take().expect("stdout is piped");
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut buf);
        String::from_utf8_lossy(&buf).into_owned()
    });

    let status = match child.wait_timeout(Duration::from_secs(timeout)) {
        Ok(Some(status)) => status,
        Ok(None) => {
            kill_group(child.id());
            let _ = child.wait();
            let output = reader.join().unwrap_or_default();
            let ms = started.elapsed().as_millis();
            return result(
                Status::Timeout,
                None,
                ms,
                output,
                Some(format!("timed out after {timeout}s")),
            );
        }
        Err(err) => {
            return result(
                Status::Fail,
                None,
                0,
                String::new(),
                Some(format!("wait failed: {err}")),
            )
        }
    };
    let output = reader.join().unwrap_or_default();
    let ms = started.elapsed().as_millis();
    let exit = status.code();
    let outcome = if status.success() {
        Status::Ok
    } else {
        Status::Fail
    };
    result(outcome, exit, ms, output, None)
}

/// Binaries a step needs: an explicit `requires` list, else the first word of
/// `run` unless the shell provides it.
pub fn required_binaries(step: &Step) -> Vec<String> {
    if let Some(explicit) = &step.requires {
        return explicit.clone();
    }
    let first = step.run.split_whitespace().next().unwrap_or_default();
    if first.is_empty() || BUILTINS.contains(&first) || first.contains('/') || first.contains('=') {
        return Vec::new();
    }
    vec![first.to_string()]
}

/// The step runs in its own process group so a timeout takes the whole tree
/// down, not just the `sh` that started it.
fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-9", "--", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn on_path(binary: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(binary).is_file())
}

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix('~') {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest.trim_start_matches('/'));
        }
    }
    PathBuf::from(path)
}
