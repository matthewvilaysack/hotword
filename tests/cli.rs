use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

struct Sandbox {
    dir: tempfile::TempDir,
}

impl Sandbox {
    fn new() -> Sandbox {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("home")).unwrap();
        fs::create_dir_all(dir.path().join("repo")).unwrap();
        Sandbox { dir }
    }

    fn home(&self) -> std::path::PathBuf {
        self.dir.path().join("home")
    }

    fn repo(&self) -> std::path::PathBuf {
        self.dir.path().join("repo")
    }

    fn run_in(&self, cwd: &Path, args: &[&str], stdin: Option<&str>) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_hotword"));
        cmd.args(args)
            .current_dir(cwd)
            .env("HOME", self.home())
            .env_remove("HOTWORD_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().unwrap();
        if let Some(text) = stdin {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(text.as_bytes())
                .unwrap();
        } else {
            drop(child.stdin.take());
        }
        child.wait_with_output().unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_in(&self.repo(), args, None)
    }
}

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn add_echo(sb: &Sandbox, name: &str, trigger: &str) {
    let output = sb.run(&[
        "add",
        name,
        "--trigger",
        trigger,
        "--step",
        "hello: echo hello from workflow",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn bare_invocation_states_the_empty_case_and_next_steps() {
    let sb = Sandbox::new();
    let output = sb.run(&[]);
    assert!(output.status.success());
    let text = out(&output);
    assert!(text.starts_with("bin: "), "{text}");
    assert!(text.contains("description: "), "{text}");
    assert!(text.contains("workflows: 0 found"), "{text}");
    assert!(
        text.contains("hotword add <name> --trigger \"<phrase>\" --step \"<name>: <command>\""),
        "{text}"
    );
}

#[test]
fn add_writes_a_user_workflow_and_listing_shows_it() {
    let sb = Sandbox::new();
    let output = sb.run(&[
        "add",
        "apple-status",
        "--trigger",
        "apple check status",
        "--step",
        "version: echo 0.1",
        "--description",
        "Where things stand",
    ]);
    let text = out(&output);
    assert!(output.status.success(), "{text}");
    let path = sb.home().join(".config/hotword/apple-status.toml");
    assert!(
        text.contains(&format!("saved: {}", path.display())),
        "{text}"
    );
    assert!(path.exists());

    let listing = out(&sb.run(&[]));
    assert!(listing.contains("workflows[1]{name,triggers,on,steps,source}:\n  apple-status,apple check status,-,1,user\n"), "{listing}");
    assert!(
        listing.contains("help[") && listing.contains("hotword run apple-status"),
        "{listing}"
    );
}

#[test]
fn add_without_steps_or_a_terminal_is_a_usage_error() {
    let sb = Sandbox::new();
    let output = sb.run(&["add", "x", "--trigger", "x"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        out(&output).contains("error: no steps given"),
        "{}",
        out(&output)
    );
}

#[test]
fn add_to_project_needs_a_hotword_dir() {
    let sb = Sandbox::new();
    let output = sb.run(&[
        "add",
        "x",
        "--trigger",
        "x",
        "--step",
        "echo x",
        "--project",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        out(&output).contains("error: no .hotword/ directory"),
        "{}",
        out(&output)
    );

    fs::create_dir_all(sb.repo().join(".hotword")).unwrap();
    let output = sb.run(&[
        "add",
        "x",
        "--trigger",
        "x",
        "--step",
        "echo x",
        "--project",
    ]);
    assert!(output.status.success());
    assert!(sb.repo().join(".hotword/x.toml").exists());
    let listing = out(&sb.run(&[]));
    assert!(listing.contains("x,x,-,1,project"), "{listing}");
}

#[test]
fn run_prints_a_report_and_json_when_asked() {
    let sb = Sandbox::new();
    add_echo(&sb, "hi", "say hi");
    let output = sb.run(&["run", "hi"]);
    assert!(output.status.success());
    let text = out(&output);
    assert!(text.contains("hotword: hi\n"), "{text}");
    assert!(text.contains("hello:\n  hello from workflow\n"), "{text}");

    let json: serde_json::Value =
        serde_json::from_str(&out(&sb.run(&["run", "hi", "--json"]))).unwrap();
    assert_eq!(json["summary"]["ok"], 1);
}

#[test]
fn run_strict_fails_when_a_step_fails() {
    let sb = Sandbox::new();
    sb.run(&["add", "bad", "--trigger", "bad", "--step", "boom: exit 4"]);
    assert!(sb.run(&["run", "bad"]).status.success());
    let strict = sb.run(&["run", "bad", "--strict"]);
    assert_eq!(strict.status.code(), Some(1));
}

#[test]
fn unknown_workflow_is_a_structured_error() {
    let sb = Sandbox::new();
    let output = sb.run(&["run", "nope"]);
    assert_eq!(output.status.code(), Some(1));
    let text = out(&output);
    assert!(text.contains("error: no workflow named nope"), "{text}");
    assert!(
        text.contains("help: run `hotword` to list workflows"),
        "{text}"
    );
}

#[test]
fn show_prints_toml_and_remove_deletes_it() {
    let sb = Sandbox::new();
    add_echo(&sb, "hi", "say hi");
    let shown = out(&sb.run(&["show", "hi"]));
    assert!(shown.contains("name = \"hi\""), "{shown}");
    assert!(
        shown.contains("run = \"echo hello from workflow\""),
        "{shown}"
    );
    let removed = sb.run(&["remove", "hi"]);
    assert!(removed.status.success());
    assert!(out(&removed).contains("removed: "), "{}", out(&removed));
    assert!(out(&sb.run(&[])).contains("workflows: 0 found"));
}

#[test]
fn match_reports_which_workflows_a_prompt_fires() {
    let sb = Sandbox::new();
    add_echo(&sb, "apple-status", "apple check status");
    add_echo(&sb, "other", "deploy legco");
    let hit = out(&sb.run(&["match", "hey, Apple Check Status please"]));
    assert!(
        hit.contains("matches[1]{name,trigger}:\n  apple-status,apple check status\n"),
        "{hit}"
    );
    let miss = out(&sb.run(&["match", "nothing here"]));
    assert!(
        miss.contains("matches: 0 workflows fire for that text"),
        "{miss}"
    );
}

#[test]
fn hook_prompt_injects_context_only_when_a_trigger_matches() {
    let sb = Sandbox::new();
    add_echo(&sb, "apple-status", "apple check status");
    let payload = format!(
        r#"{{"prompt":"apple check status","cwd":"{}"}}"#,
        sb.repo().display()
    );
    let output = sb.run_in(&sb.repo(), &["hook", "prompt"], Some(&payload));
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&out(&output)).unwrap();
    assert_eq!(
        json["hookSpecificOutput"]["hookEventName"],
        "UserPromptSubmit"
    );
    let ctx = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.contains("hotword: apple-status"), "{ctx}");
    assert!(ctx.contains("hello from workflow"), "{ctx}");

    let quiet = sb.run_in(
        &sb.repo(),
        &["hook", "prompt"],
        Some(r#"{"prompt":"unrelated"}"#),
    );
    assert!(quiet.status.success());
    assert_eq!(out(&quiet), "");
}

#[test]
fn hook_prompt_finds_project_workflows_from_the_payload_cwd() {
    let sb = Sandbox::new();
    fs::create_dir_all(sb.repo().join(".hotword")).unwrap();
    fs::write(
        sb.repo().join(".hotword/repo-check.toml"),
        "name = \"repo-check\"\ntriggers = [\"repo check\"]\n[[steps]]\nname = \"where\"\nrun = \"pwd\"\n",
    )
    .unwrap();
    let payload = format!(
        r#"{{"prompt":"repo check","cwd":"{}"}}"#,
        sb.repo().display()
    );
    let output = sb.run_in(&sb.home(), &["hook", "prompt"], Some(&payload));
    let json: serde_json::Value = serde_json::from_str(&out(&output)).unwrap();
    let ctx = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.contains("hotword: repo-check"), "{ctx}");
    let repo = fs::canonicalize(sb.repo()).unwrap();
    assert!(ctx.contains(&repo.display().to_string()), "{ctx}");
}

#[test]
fn hook_session_start_runs_only_session_workflows() {
    let sb = Sandbox::new();
    add_echo(&sb, "prompt-only", "prompt only");
    let output = sb.run(&[
        "add",
        "boot",
        "--on",
        "session-start",
        "--step",
        "hi: echo booted",
    ]);
    assert!(output.status.success(), "{}", out(&output));
    let output = sb.run_in(
        &sb.repo(),
        &["hook", "session-start"],
        Some(r#"{"source":"startup"}"#),
    );
    let json: serde_json::Value = serde_json::from_str(&out(&output)).unwrap();
    assert_eq!(json["hookSpecificOutput"]["hookEventName"], "SessionStart");
    let ctx = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(
        ctx.contains("hotword: boot") && ctx.contains("booted"),
        "{ctx}"
    );
    assert!(!ctx.contains("prompt-only"), "{ctx}");
}

#[test]
fn hook_with_broken_stdin_stays_silent_and_exits_zero() {
    let sb = Sandbox::new();
    let output = sb.run_in(&sb.repo(), &["hook", "prompt"], Some("not json"));
    assert!(output.status.success());
    assert_eq!(out(&output), "");
    assert!(String::from_utf8_lossy(&output.stderr).contains("hotword:"));
}

#[test]
fn install_and_uninstall_edit_the_agent_settings() {
    let sb = Sandbox::new();
    let output = sb.run(&["install", "--agent", "claude"]);
    assert!(output.status.success(), "{}", out(&output));
    let text = out(&output);
    let settings = sb.home().join(".claude/settings.json");
    assert!(
        text.contains(&format!("settings: {}", settings.display())),
        "{text}"
    );
    assert!(
        text.contains("changes[2]:\n  added UserPromptSubmit hook\n  added SessionStart hook\n"),
        "{text}"
    );
    assert!(settings.exists());

    let again = out(&sb.run(&["install", "--agent", "claude"]));
    assert!(
        again.contains("changes: none, already installed"),
        "{again}"
    );

    let dry = sb.run(&["install", "--agent", "codex", "--project", "--dry-run"]);
    assert!(out(&dry).contains("dry run"), "{}", out(&dry));
    assert!(!sb.repo().join(".codex/hooks.json").exists());

    let removed = out(&sb.run(&["uninstall", "--agent", "claude"]));
    assert!(
        removed.contains("removed UserPromptSubmit hook"),
        "{removed}"
    );
    let value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&settings).unwrap()).unwrap();
    assert!(value.get("hooks").is_none(), "{value}");
}

#[test]
fn run_and_hook_pass_the_prompt_to_steps() {
    let sb = Sandbox::new();
    let output = sb.run(&[
        "add",
        "pr",
        "--trigger",
        "review pr",
        "--step",
        "which: echo \"$HOTWORD_PROMPT\" | grep -oE '[0-9]+'",
    ]);
    assert!(output.status.success(), "{}", out(&output));
    let text = out(&sb.run(&["run", "pr", "--prompt", "review pr 42 please"]));
    assert!(text.contains("which:\n  42\n"), "{text}");

    let payload = r#"{"prompt":"Review PR 7 for me"}"#;
    let hook = sb.run_in(&sb.repo(), &["hook", "prompt"], Some(payload));
    let json: serde_json::Value = serde_json::from_str(&out(&hook)).unwrap();
    let ctx = json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.contains("which:\n  7\n"), "{ctx}");
}

#[test]
fn list_json_is_machine_readable_for_other_front_ends() {
    let sb = Sandbox::new();
    add_echo(&sb, "apple-status", "apple check status");
    let output = sb.run(&["list", "--json"]);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&out(&output)).unwrap();
    assert_eq!(json["workflows"][0]["name"], "apple-status");
    assert_eq!(json["workflows"][0]["triggers"][0], "apple check status");
    assert_eq!(json["workflows"][0]["source"], "user");
    assert_eq!(json["workflows"][0]["steps"], 1);
    assert!(json["workflows"][0]["path"]
        .as_str()
        .unwrap()
        .ends_with("apple-status.toml"));
    assert!(json["user_dir"].as_str().is_some());

    let plain = out(&sb.run(&["list"]));
    assert!(
        plain.contains("workflows[1]{name,triggers,on,steps,source}:"),
        "{plain}"
    );
}

#[test]
fn save_reads_toml_from_stdin_validates_and_writes_it() {
    let sb = Sandbox::new();
    let toml = "name = \"from-app\"\ntriggers = [\"from app\"]\n\n[[steps]]\nname = \"hi\"\nrun = \"echo hi\"\n";
    let output = sb.run_in(&sb.repo(), &["save"], Some(toml));
    assert!(output.status.success(), "{}", out(&output));
    let path = sb.home().join(".config/hotword/from-app.toml");
    assert!(
        out(&output).contains(&format!("saved: {}", path.display())),
        "{}",
        out(&output)
    );
    assert!(fs::read_to_string(&path)
        .unwrap()
        .contains("name = \"from-app\""));

    let again = sb.run_in(
        &sb.repo(),
        &["save"],
        Some(&toml.replace("echo hi", "echo again")),
    );
    assert!(again.status.success(), "save overwrites: {}", out(&again));
    assert!(fs::read_to_string(&path).unwrap().contains("echo again"));

    let bad = sb.run_in(&sb.repo(), &["save"], Some("name = \"Bad Name\"\n"));
    assert_eq!(bad.status.code(), Some(1));
    assert!(out(&bad).contains("error:"), "{}", out(&bad));
    assert!(!sb.home().join(".config/hotword/Bad Name.toml").exists());
}

#[test]
fn history_records_runs_once_initialised_and_lists_changes() {
    if !hotword::runner::on_path("dolt") {
        eprintln!("dolt not on PATH, skipping");
        return;
    }
    let sb = Sandbox::new();
    let marker = sb.dir.path().join("marker");
    let step = format!("mark: cat {} 2>/dev/null || echo none", marker.display());
    sb.run(&[
        "add",
        "repo-status",
        "--trigger",
        "repo status",
        "--step",
        &step,
    ]);

    let before = out(&sb.run(&["history"]));
    assert!(before.contains("history: not initialised"), "{before}");
    assert!(before.contains("hotword history init"), "{before}");

    let init = sb.run(&["history", "init"]);
    assert!(init.status.success(), "{}", out(&init));
    assert!(sb.home().join(".config/hotword/history/.dolt").is_dir());

    assert!(sb.run(&["run", "repo-status"]).status.success());
    fs::write(&marker, "changed\n").unwrap();
    let payload = format!(
        r#"{{"prompt":"repo status please","cwd":"{}"}}"#,
        sb.repo().display()
    );
    assert!(sb
        .run_in(&sb.repo(), &["hook", "prompt"], Some(&payload))
        .status
        .success());

    let listing = out(&sb.run(&["history"]));
    assert!(
        listing.contains("runs[2]{workflow,started,host,ok,fail,skip,timeout,prompt}:"),
        "{listing}"
    );
    assert!(listing.contains("repo status please"), "{listing}");

    let changes = out(&sb.run(&["history", "changes", "repo-status"]));
    assert!(
        changes.contains("changes[1]{step,before,after,output}:"),
        "{changes}"
    );
    assert!(changes.contains("mark,ok,ok,changed"), "{changes}");
    assert!(changes.contains("  changed\n"), "{changes}");

    let none = out(&sb.run(&["history", "changes", "never-ran"]));
    assert!(
        none.contains("changes: fewer than two runs of never-ran recorded"),
        "{none}"
    );
}

#[test]
fn doctor_runs_the_workflow_and_prints_guided_fixes() {
    let sb = Sandbox::new();
    let output = sb.run(&[
        "add",
        "broken",
        "--trigger",
        "broken",
        "--step",
        "tool: definitely-not-a-binary-xyz --flag",
        "--step",
        "quiet: grep zzz /dev/null",
        "--step",
        "fine: echo ok",
    ]);
    assert!(output.status.success(), "{}", out(&output));
    let text = out(&sb.run(&["doctor", "broken"]));
    assert!(text.contains("doctor: broken"), "{text}");
    assert!(text.contains("problems: 2 of 3 steps"), "{text}");
    assert!(text.contains("steps[3]{name,status,verdict}:"), "{text}");
    assert!(text.contains("tool,skip,missing-tool"), "{text}");
    assert!(text.contains("quiet,fail,silent-failure"), "{text}");
    assert!(text.contains("fine,ok,healthy"), "{text}");
    assert!(text.contains("start with: tool"), "{text}");
    assert!(text.contains("fix:"), "{text}");

    let json: serde_json::Value =
        serde_json::from_str(&out(&sb.run(&["doctor", "broken", "--json"]))).unwrap();
    assert_eq!(json["steps"][0]["verdict"], "missing-tool");
    assert_eq!(json["first_problem"], "tool");

    let one = out(&sb.run(&["doctor", "broken", "--step", "fine", "--json"]));
    let json: serde_json::Value = serde_json::from_str(&one).unwrap();
    assert_eq!(json["steps"].as_array().unwrap().len(), 1);
    assert_eq!(json["steps"][0]["name"], "fine");
    assert_eq!(json["steps"][0]["output"].as_str().unwrap().trim(), "ok");

    let missing = sb.run(&["doctor", "broken", "--step", "nope"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        out(&missing).contains("error: no step named nope"),
        "{}",
        out(&missing)
    );
}
