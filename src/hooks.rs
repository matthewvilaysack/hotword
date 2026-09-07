//! Registers `hotword hook ...` as command hooks in an agent's settings file.
//! Claude Code (`settings.json`) and Codex (`hooks.json`) share the same
//! `hooks.<Event>[].hooks[]` shape, so one editor serves both.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
}

/// Generous because a workflow runs several commands; UserPromptSubmit defaults
/// to 30s in Claude Code and a timed-out hook's context is discarded.
const HOOK_TIMEOUT_SECS: u64 = 120;

struct HookSpec {
    event: &'static str,
    subcommand: &'static str,
    matcher: Option<&'static str>,
}

const SPECS: [HookSpec; 2] = [
    HookSpec {
        event: "UserPromptSubmit",
        subcommand: "hook prompt",
        matcher: None,
    },
    HookSpec {
        event: "SessionStart",
        subcommand: "hook session-start",
        matcher: Some("startup|resume|clear"),
    },
];

pub fn settings_path(agent: Agent, scope: Scope, home: &Path, project_root: &Path) -> PathBuf {
    let base = match scope {
        Scope::User => home,
        Scope::Project => project_root,
    };
    match agent {
        Agent::Claude => base.join(".claude").join("settings.json"),
        Agent::Codex => base.join(".codex").join("hooks.json"),
    }
}

/// The command prefix hooks should use: the bare name when PATH resolves it to
/// this executable, else the absolute path.
pub fn bin_command() -> String {
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| fs::canonicalize(p).ok());
    let on_path = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join("hotword"))
            .find(|candidate| candidate.is_file())
            .and_then(|found| fs::canonicalize(found).ok())
    });
    match (exe, on_path) {
        (Some(exe), Some(found)) if exe == found => "hotword".to_string(),
        (Some(exe), _) => exe.display().to_string(),
        (None, _) => "hotword".to_string(),
    }
}

pub fn install(path: &Path, bin: &str) -> Result<Vec<String>> {
    let mut root = read_settings(path)?;
    let mut changes = Vec::new();
    for spec in &SPECS {
        let wanted = format!("{bin} {}", spec.subcommand);
        let groups = event_groups(&mut root, spec.event);
        match find_ours(groups, spec.subcommand) {
            Some(handler) if handler["command"] == wanted => {}
            Some(handler) => {
                handler["command"] = Value::String(wanted);
                changes.push(format!("updated {} hook path", spec.event));
            }
            None => {
                let mut group = Map::new();
                if let Some(matcher) = spec.matcher {
                    group.insert("matcher".into(), Value::String(matcher.into()));
                }
                group.insert(
                    "hooks".into(),
                    json!([{ "type": "command", "command": wanted, "timeout": HOOK_TIMEOUT_SECS }]),
                );
                groups.push(Value::Object(group));
                changes.push(format!("added {} hook", spec.event));
            }
        }
    }
    if !changes.is_empty() {
        write_settings(path, &root)?;
    }
    Ok(changes)
}

pub fn uninstall(path: &Path) -> Result<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut root = read_settings(path)?;
    let mut changes = Vec::new();
    for spec in &SPECS {
        let groups = event_groups(&mut root, spec.event);
        let before = groups.len();
        for group in groups.iter_mut() {
            if let Some(handlers) = group["hooks"].as_array_mut() {
                handlers.retain(|h| !is_ours(h, spec.subcommand));
            }
        }
        groups.retain(|g| g["hooks"].as_array().is_some_and(|h| !h.is_empty()));
        if groups.len() != before
            || groups
                .iter()
                .any(|g| g["hooks"].as_array().is_some_and(|h| h.is_empty()))
        {
            changes.push(format!("removed {} hook", spec.event));
        }
        if groups.is_empty() {
            root["hooks"]
                .as_object_mut()
                .map(|hooks| hooks.remove(spec.event));
        }
    }
    if root["hooks"].as_object().is_some_and(|h| h.is_empty()) {
        root.as_object_mut().map(|o| o.remove("hooks"));
    }
    if !changes.is_empty() {
        write_settings(path, &root)?;
    }
    Ok(changes)
}

fn read_settings(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("{} is not valid JSON", path.display()))?;
    anyhow::ensure!(value.is_object(), "{} is not a JSON object", path.display());
    Ok(value)
}

fn write_settings(path: &Path, root: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(root)? + "\n";
    fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

fn event_groups<'a>(root: &'a mut Value, event: &str) -> &'a mut Vec<Value> {
    let hooks = root
        .as_object_mut()
        .expect("settings root is an object")
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let groups = hooks
        .as_object_mut()
        .expect("hooks is an object")
        .entry(event)
        .or_insert_with(|| json!([]));
    if !groups.is_array() {
        *groups = json!([]);
    }
    groups.as_array_mut().expect("event groups is an array")
}

fn find_ours<'a>(groups: &'a mut [Value], subcommand: &str) -> Option<&'a mut Value> {
    groups
        .iter_mut()
        .filter_map(|g| g["hooks"].as_array_mut())
        .flat_map(|handlers| handlers.iter_mut())
        .find(|h| is_ours(h, subcommand))
}

fn is_ours(handler: &Value, subcommand: &str) -> bool {
    let Some(command) = handler["command"].as_str() else {
        return false;
    };
    let Some(prefix) = command.strip_suffix(subcommand) else {
        return false;
    };
    prefix.trim_end().ends_with("hotword")
}
