use std::fs;
use std::path::Path;

use hotword::hooks::{install, settings_path, uninstall, Agent, Scope};

fn read(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn commands(value: &serde_json::Value, event: &str) -> Vec<String> {
    value["hooks"][event]
        .as_array()
        .map(|groups| {
            groups
                .iter()
                .flat_map(|g| g["hooks"].as_array().cloned().unwrap_or_default())
                .map(|h| h["command"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn settings_paths_follow_each_agent_convention() {
    let home = Path::new("/h");
    let root = Path::new("/r");
    assert_eq!(
        settings_path(Agent::Claude, Scope::User, home, root),
        Path::new("/h/.claude/settings.json")
    );
    assert_eq!(
        settings_path(Agent::Claude, Scope::Project, home, root),
        Path::new("/r/.claude/settings.json")
    );
    assert_eq!(
        settings_path(Agent::Codex, Scope::User, home, root),
        Path::new("/h/.codex/hooks.json")
    );
    assert_eq!(
        settings_path(Agent::Codex, Scope::Project, home, root),
        Path::new("/r/.codex/hooks.json")
    );
}

#[test]
fn install_creates_the_file_with_both_hooks() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join(".claude/settings.json");
    let changes = install(&path, "hotword").unwrap();
    assert_eq!(
        changes,
        vec!["added UserPromptSubmit hook", "added SessionStart hook"]
    );
    let value = read(&path);
    assert_eq!(
        commands(&value, "UserPromptSubmit"),
        vec!["hotword hook prompt"]
    );
    assert_eq!(
        commands(&value, "SessionStart"),
        vec!["hotword hook session-start"]
    );
    assert_eq!(
        value["hooks"]["SessionStart"][0]["matcher"],
        "startup|resume|clear"
    );
    assert_eq!(
        value["hooks"]["UserPromptSubmit"][0]["hooks"][0]["timeout"],
        120
    );
}

#[test]
fn install_twice_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    install(&path, "hotword").unwrap();
    let before = fs::read_to_string(&path).unwrap();
    let changes = install(&path, "hotword").unwrap();
    assert!(changes.is_empty(), "{changes:?}");
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
}

#[test]
fn install_repairs_a_moved_binary() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    install(&path, "/old/hotword").unwrap();
    let changes = install(&path, "/new/hotword").unwrap();
    assert_eq!(
        changes,
        vec![
            "updated UserPromptSubmit hook path",
            "updated SessionStart hook path"
        ]
    );
    let value = read(&path);
    assert_eq!(
        commands(&value, "UserPromptSubmit"),
        vec!["/new/hotword hook prompt"]
    );
    assert_eq!(
        commands(&value, "SessionStart"),
        vec!["/new/hotword hook session-start"]
    );
}

#[test]
fn install_and_uninstall_leave_other_settings_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    fs::write(
        &path,
        r#"{"model":"opus","hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command","command":"bash other.sh"}]}],"Stop":[{"hooks":[{"type":"command","command":"bash stop.sh"}]}]}}"#,
    )
    .unwrap();
    install(&path, "hotword").unwrap();
    let value = read(&path);
    assert_eq!(value["model"], "opus");
    assert_eq!(
        commands(&value, "UserPromptSubmit"),
        vec!["bash other.sh", "hotword hook prompt"]
    );
    assert_eq!(commands(&value, "Stop"), vec!["bash stop.sh"]);

    let changes = uninstall(&path).unwrap();
    assert_eq!(
        changes,
        vec!["removed UserPromptSubmit hook", "removed SessionStart hook"]
    );
    let value = read(&path);
    assert_eq!(value["model"], "opus");
    assert_eq!(commands(&value, "UserPromptSubmit"), vec!["bash other.sh"]);
    assert_eq!(commands(&value, "Stop"), vec!["bash stop.sh"]);
    assert!(value["hooks"].get("SessionStart").is_none());
    assert!(uninstall(&path).unwrap().is_empty());
}

#[test]
fn uninstall_on_a_missing_file_is_a_no_op() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("nope.json");
    assert!(uninstall(&path).unwrap().is_empty());
    assert!(!path.exists());
}
