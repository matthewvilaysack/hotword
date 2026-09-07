use std::fs;

use hotword::workflow::{Event, Source, Store, Workflow};

const MINIMAL: &str = r#"
name = "deploy-status"
triggers = ["check the deploy"]

[[steps]]
name = "version"
run = "mytool --version"
"#;

#[test]
fn parses_minimal_toml_with_defaults() {
    let wf = Workflow::from_toml(MINIMAL).unwrap();
    assert_eq!(wf.name, "deploy-status");
    assert_eq!(wf.triggers, vec!["check the deploy"]);
    assert!(wf.on.is_empty());
    assert_eq!(wf.timeout, 30);
    assert_eq!(wf.max_lines, 40);
    assert_eq!(wf.steps.len(), 1);
    assert_eq!(wf.steps[0].run, "mytool --version");
    assert_eq!(wf.steps[0].timeout, None);
}

#[test]
fn parses_session_start_event() {
    let text = format!("{MINIMAL}\n").replace("triggers", "on = [\"session-start\"]\ntriggers");
    let wf = Workflow::from_toml(&text).unwrap();
    assert_eq!(wf.on, vec![Event::SessionStart]);
}

#[test]
fn toml_round_trips() {
    let wf = Workflow::from_toml(MINIMAL).unwrap();
    let again = Workflow::from_toml(&wf.to_toml()).unwrap();
    assert_eq!(wf, again);
}

#[test]
fn matches_trigger_ignoring_case_and_spacing() {
    let wf = Workflow::from_toml(MINIMAL).unwrap();
    assert!(wf.matches("Check   THE deploy please"));
    assert!(!wf.matches("deploy the check"));
}

#[test]
fn rejects_workflow_without_steps() {
    let err = Workflow::from_toml("name = \"x\"\ntriggers = [\"x\"]\n").unwrap_err();
    assert!(err.to_string().contains("at least one step"), "{err}");
}

#[test]
fn rejects_name_that_is_not_a_slug() {
    let text = MINIMAL.replace("deploy-status", "Deploy Status");
    let err = Workflow::from_toml(&text).unwrap_err();
    assert!(err.to_string().contains("lowercase"), "{err}");
}

#[test]
fn rejects_workflow_with_no_trigger_and_no_event() {
    let text = MINIMAL.replace("triggers = [\"check the deploy\"]", "");
    let err = Workflow::from_toml(&text).unwrap_err();
    assert!(err.to_string().contains("trigger"), "{err}");
}

#[test]
fn discover_finds_project_dir_walking_up() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("repo");
    fs::create_dir_all(root.join(".hotword")).unwrap();
    let nested = root.join("a/b");
    fs::create_dir_all(&nested).unwrap();
    let home = tmp.path().join("home");
    let store = Store::discover(&nested, &home);
    assert_eq!(store.project_dir, Some(root.join(".hotword")));
    assert_eq!(store.user_dir, home.join(".config/hotword"));
}

#[test]
fn project_workflow_shadows_user_workflow_of_same_name() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store {
        user_dir: tmp.path().join("user"),
        project_dir: Some(tmp.path().join("proj")),
    };
    let user = Workflow::from_toml(MINIMAL).unwrap();
    let mut proj = user.clone();
    proj.steps[0].run = "echo project".into();
    store.save(&user, Source::User).unwrap();
    store.save(&proj, Source::Project).unwrap();
    let other = Workflow::from_toml(&MINIMAL.replace("deploy-status", "aaa")).unwrap();
    store.save(&other, Source::User).unwrap();

    let all = store.load_all().unwrap();
    let names: Vec<_> = all.iter().map(|l| l.workflow.name.as_str()).collect();
    assert_eq!(names, vec!["aaa", "deploy-status"]);
    let found = store.find("deploy-status").unwrap().unwrap();
    assert_eq!(found.source, Source::Project);
    assert_eq!(found.workflow.steps[0].run, "echo project");
}

#[test]
fn save_and_remove_use_name_as_filename() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store {
        user_dir: tmp.path().join("user"),
        project_dir: None,
    };
    let wf = Workflow::from_toml(MINIMAL).unwrap();
    let path = store.save(&wf, Source::User).unwrap();
    assert_eq!(path, tmp.path().join("user/deploy-status.toml"));
    assert!(path.exists());
    let removed = store.remove("deploy-status").unwrap();
    assert_eq!(removed, path);
    assert!(!path.exists());
    assert!(store.remove("deploy-status").is_err());
}

#[test]
fn save_to_project_without_project_dir_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store {
        user_dir: tmp.path().join("user"),
        project_dir: None,
    };
    let wf = Workflow::from_toml(MINIMAL).unwrap();
    let err = store.save(&wf, Source::Project).unwrap_err();
    assert!(err.to_string().contains(".hotword"), "{err}");
}

#[test]
fn load_reports_the_broken_file_by_path() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store {
        user_dir: tmp.path().to_path_buf(),
        project_dir: None,
    };
    fs::write(tmp.path().join("bad.toml"), "name = 3").unwrap();
    let err = store.load_all().unwrap_err();
    assert!(err.to_string().contains("bad.toml"), "{err}");
}
