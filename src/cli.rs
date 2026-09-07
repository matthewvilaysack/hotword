//! The `hotword` command. Output goes to stdout in a compact, agent-readable
//! shape; diagnostics go to stderr. Exit 0 on success, 1 on error, 2 on usage.

use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::hooks::{self, Agent, Scope};
use crate::report::{hook_json, render_json, render_text};
use crate::runner::{self, Report, Status};
use crate::workflow::{Event, Loaded, Source, Step, Store, Workflow, PROJECT_DIR};

const DESCRIPTION: &str =
    "Run a workflow of shell steps when you type a phrase, or when a coding agent session starts";

#[derive(Parser)]
#[command(name = "hotword", version, about = DESCRIPTION, disable_help_subcommand = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// List workflows (what `hotword` alone prints), optionally as JSON
    List {
        #[arg(long)]
        json: bool,
    },
    /// Run a workflow now and print its report
    Run(RunArgs),
    /// Print a workflow's TOML
    Show { name: String },
    /// Create a workflow (interactive when no --step is given on a terminal)
    Add(AddArgs),
    /// Open a workflow's TOML in $EDITOR
    Edit { name: String },
    /// Delete a workflow
    Remove { name: String },
    /// Show which workflows a piece of text would fire
    Match { text: String },
    /// Register the hooks in an agent's settings file
    Install(InstallArgs),
    /// Remove the hooks from an agent's settings file
    Uninstall(InstallArgs),
    /// Entry point the agent's hook calls (reads the event JSON on stdin)
    Hook {
        #[arg(value_enum)]
        event: HookEvent,
    },
}

#[derive(Args)]
struct RunArgs {
    name: String,
    /// Emit JSON instead of text
    #[arg(long)]
    json: bool,
    /// Do not truncate step output
    #[arg(long)]
    full: bool,
    /// Exit 1 when any step fails or times out
    #[arg(long)]
    strict: bool,
    /// Text to expose to steps as HOTWORD_PROMPT, as if it had fired the workflow
    #[arg(long)]
    prompt: Option<String>,
}

#[derive(Args)]
struct AddArgs {
    /// Lowercase slug, like apple-status
    name: String,
    /// Phrase that fires the workflow when it appears in a prompt (repeatable)
    #[arg(long = "trigger")]
    triggers: Vec<String>,
    /// Session event that fires the workflow (repeatable)
    #[arg(long = "on", value_enum)]
    on: Vec<EventArg>,
    /// A step as "name: command" or just "command" (repeatable, runs in order)
    #[arg(long = "step")]
    steps: Vec<String>,
    #[arg(long)]
    description: Option<String>,
    /// Save into this repo's .hotword/ instead of your user config
    #[arg(long)]
    project: bool,
    /// Per-step timeout in seconds
    #[arg(long, default_value_t = crate::workflow::DEFAULT_TIMEOUT)]
    timeout: u64,
}

#[derive(Args)]
struct InstallArgs {
    #[arg(long, value_enum, default_value_t = AgentArg::Claude)]
    agent: AgentArg,
    /// Edit the repo's settings file instead of your user one
    #[arg(long)]
    project: bool,
    /// Show what would change without writing
    #[arg(long)]
    dry_run: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum AgentArg {
    Claude,
    Codex,
}

#[derive(Clone, Copy, ValueEnum)]
enum EventArg {
    SessionStart,
}

#[derive(Clone, Copy, ValueEnum)]
enum HookEvent {
    Prompt,
    SessionStart,
}

pub fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli.command) {
        Ok(code) => code,
        Err(err) => {
            println!("error: {err:#}");
            if let Some(help) = help_for(&err) {
                println!("help: {help}");
            }
            ExitCode::from(1)
        }
    }
}

fn dispatch(command: Option<Cmd>) -> Result<ExitCode> {
    let cwd = std::env::current_dir().context("reading the current directory")?;
    match command {
        None => list(&store_for(&cwd)),
        Some(Cmd::List { json: false }) => list(&store_for(&cwd)),
        Some(Cmd::List { json: true }) => list_json(&store_for(&cwd)),
        Some(Cmd::Run(args)) => run(&store_for(&cwd), &cwd, args),
        Some(Cmd::Show { name }) => show(&store_for(&cwd), &name),
        Some(Cmd::Add(args)) => add(&store_for(&cwd), args),
        Some(Cmd::Edit { name }) => edit(&store_for(&cwd), &name),
        Some(Cmd::Remove { name }) => remove(&store_for(&cwd), &name),
        Some(Cmd::Match { text }) => match_text(&store_for(&cwd), &text),
        Some(Cmd::Install(args)) => install(&cwd, args, true),
        Some(Cmd::Uninstall(args)) => install(&cwd, args, false),
        Some(Cmd::Hook { event }) => hook(event),
    }
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// HOTWORD_HOME overrides the user workflow directory, which is what tests use.
fn store_for(cwd: &Path) -> Store {
    let mut store = Store::discover(cwd, &home());
    if let Some(dir) = std::env::var_os("HOTWORD_HOME") {
        store.user_dir = PathBuf::from(dir);
    }
    store
}

fn help_for(err: &anyhow::Error) -> Option<&'static str> {
    let text = err.to_string();
    if text.starts_with("no workflow named") {
        Some("run `hotword` to list workflows")
    } else if text.contains(PROJECT_DIR) {
        Some("or drop --project to save to your user config")
    } else {
        None
    }
}

fn list(store: &Store) -> Result<ExitCode> {
    let all = store.load_all()?;
    println!(
        "bin: {}",
        collapse_home(&std::env::current_exe().unwrap_or_default())
    );
    println!("description: {DESCRIPTION}");
    println!("user_dir: {}", collapse_home(&store.user_dir));
    match &store.project_dir {
        Some(dir) => println!("project_dir: {}", collapse_home(dir)),
        None => println!("project_dir: none (no {PROJECT_DIR}/ between here and /)"),
    }
    if all.is_empty() {
        println!("workflows: 0 found");
        println!("help[2]:");
        println!("  Run `hotword add <name> --trigger \"<phrase>\" --step \"<name>: <command>\"` to create one");
        println!("  Run `hotword add <name>` on a terminal for the guided version");
        return Ok(ExitCode::SUCCESS);
    }
    println!("workflows[{}]{{name,triggers,on,steps,source}}:", all.len());
    for loaded in &all {
        let wf = &loaded.workflow;
        println!(
            "  {},{},{},{},{}",
            wf.name,
            join_or_dash(&wf.triggers),
            join_or_dash(
                &wf.on
                    .iter()
                    .map(|e| e.as_str().to_string())
                    .collect::<Vec<_>>()
            ),
            wf.steps.len(),
            loaded.source.as_str()
        );
    }
    let first = &all[0].workflow.name;
    println!("help[3]:");
    println!("  Run `hotword run {first}` to run one now");
    println!("  Run `hotword show {first}` to see its steps");
    println!("  Run `hotword install` to fire them from your agent's hooks");
    Ok(ExitCode::SUCCESS)
}

/// The listing for other front ends, such as a desktop app, so they never
/// parse TOML themselves.
fn list_json(store: &Store) -> Result<ExitCode> {
    let all = store.load_all()?;
    let workflows: Vec<serde_json::Value> = all
        .iter()
        .map(|l| {
            serde_json::json!({
                "name": l.workflow.name,
                "description": l.workflow.description,
                "triggers": l.workflow.triggers,
                "on": l.workflow.on.iter().map(|e| e.as_str()).collect::<Vec<_>>(),
                "steps": l.workflow.steps.len(),
                "source": l.source.as_str(),
                "path": l.path,
            })
        })
        .collect();
    let value = serde_json::json!({
        "user_dir": store.user_dir,
        "project_dir": store.project_dir,
        "workflows": workflows,
    });
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(ExitCode::SUCCESS)
}

fn join_or_dash(items: &[String]) -> String {
    if items.is_empty() {
        "-".to_string()
    } else {
        items.join("|")
    }
}

fn collapse_home(path: &Path) -> String {
    let text = path.display().to_string();
    match home().to_str() {
        Some(home) if !home.is_empty() && text.starts_with(home) => {
            format!("~{}", &text[home.len()..])
        }
        _ => text,
    }
}

fn require(store: &Store, name: &str) -> Result<Loaded> {
    store
        .find(name)?
        .ok_or_else(|| anyhow!("no workflow named {name}"))
}

fn run(store: &Store, cwd: &Path, args: RunArgs) -> Result<ExitCode> {
    let loaded = require(store, &args.name)?;
    let report = match &args.prompt {
        Some(prompt) => runner::run_for_prompt(&loaded.workflow, cwd, prompt),
        None => runner::run(&loaded.workflow, cwd),
    };
    if args.json {
        println!("{}", render_json(&report));
    } else {
        print!("{}", render_text(&report, args.full));
    }
    let failed = report.count(Status::Fail) + report.count(Status::Timeout) > 0;
    Ok(if args.strict && failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

fn show(store: &Store, name: &str) -> Result<ExitCode> {
    let loaded = require(store, name)?;
    println!("path: {}", collapse_home(&loaded.path));
    println!("source: {}", loaded.source.as_str());
    println!();
    print!("{}", loaded.workflow.to_toml());
    Ok(ExitCode::SUCCESS)
}

fn add(store: &Store, args: AddArgs) -> Result<ExitCode> {
    let source = if args.project {
        Source::Project
    } else {
        Source::User
    };
    let interactive = args.steps.is_empty() && std::io::stdin().is_terminal();
    let workflow = if interactive {
        crate::interactive::build(&args.name, args.description.clone(), args.timeout)?
    } else {
        if args.steps.is_empty() {
            println!("error: no steps given");
            println!("help: pass --step \"<name>: <command>\" at least once, or run on a terminal for the guided version");
            return Ok(ExitCode::from(2));
        }
        Workflow {
            name: args.name.clone(),
            description: args.description,
            triggers: args.triggers,
            on: args
                .on
                .iter()
                .map(|e| match e {
                    EventArg::SessionStart => Event::SessionStart,
                })
                .collect(),
            timeout: args.timeout,
            max_lines: crate::workflow::DEFAULT_MAX_LINES,
            steps: args.steps.iter().map(|s| parse_step(s)).collect(),
        }
    };
    if let Some(existing) = store.find(&workflow.name)? {
        if existing.source == source {
            bail!(
                "workflow {} already exists at {}; edit or remove it first",
                workflow.name,
                existing.path.display()
            );
        }
    }
    let path = store.save(&workflow, source)?;
    println!("saved: {}", path.display());
    println!("help[2]:");
    println!("  Run `hotword run {}` to try it", workflow.name);
    println!("  Run `hotword install` if the hooks are not registered yet");
    Ok(ExitCode::SUCCESS)
}

/// "name: command" or "command"; the bare form is named after its first word.
pub fn parse_step(text: &str) -> Step {
    let (name, run) = match text.split_once(": ") {
        Some((name, run)) if !name.contains(' ') => {
            (name.trim().to_string(), run.trim().to_string())
        }
        _ => {
            let first = text.split_whitespace().next().unwrap_or("step");
            (
                first.rsplit('/').next().unwrap_or(first).to_string(),
                text.trim().to_string(),
            )
        }
    };
    Step {
        name,
        run,
        timeout: None,
        cwd: None,
        requires: None,
    }
}

fn edit(store: &Store, name: &str) -> Result<ExitCode> {
    let loaded = require(store, name)?;
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("hotword-edit")
        .arg(&loaded.path)
        .status()
        .with_context(|| format!("starting {editor}"))?;
    if !status.success() {
        bail!("{editor} exited with {status}");
    }
    let text = std::fs::read_to_string(&loaded.path)?;
    Workflow::from_toml(&text)
        .with_context(|| format!("{} no longer parses", loaded.path.display()))?;
    println!("saved: {}", loaded.path.display());
    Ok(ExitCode::SUCCESS)
}

fn remove(store: &Store, name: &str) -> Result<ExitCode> {
    let path = store.remove(name)?;
    println!("removed: {}", path.display());
    Ok(ExitCode::SUCCESS)
}

fn match_text(store: &Store, text: &str) -> Result<ExitCode> {
    let hits: Vec<(String, String)> = store
        .load_all()?
        .into_iter()
        .filter_map(|l| {
            let trigger = l
                .workflow
                .triggers
                .iter()
                .find(|t| l.workflow.matches_trigger(t, text))?
                .clone();
            Some((l.workflow.name, trigger))
        })
        .collect();
    if hits.is_empty() {
        println!("matches: 0 workflows fire for that text");
        return Ok(ExitCode::SUCCESS);
    }
    println!("matches[{}]{{name,trigger}}:", hits.len());
    for (name, trigger) in hits {
        println!("  {name},{trigger}");
    }
    Ok(ExitCode::SUCCESS)
}

fn install(cwd: &Path, args: InstallArgs, installing: bool) -> Result<ExitCode> {
    let agent = match args.agent {
        AgentArg::Claude => Agent::Claude,
        AgentArg::Codex => Agent::Codex,
    };
    let scope = if args.project {
        Scope::Project
    } else {
        Scope::User
    };
    let path = hooks::settings_path(agent, scope, &home(), cwd);
    let bin = hooks::bin_command();
    println!("settings: {}", path.display());
    println!("command: {bin}");
    if args.dry_run {
        let verb = if installing {
            "would register"
        } else {
            "would remove"
        };
        println!("changes: dry run, {verb} the UserPromptSubmit and SessionStart hooks");
        return Ok(ExitCode::SUCCESS);
    }
    let changes = if installing {
        hooks::install(&path, &bin)?
    } else {
        hooks::uninstall(&path)?
    };
    if changes.is_empty() {
        let state = if installing {
            "already installed"
        } else {
            "nothing to remove"
        };
        println!("changes: none, {state}");
    } else {
        println!("changes[{}]:", changes.len());
        for change in changes {
            println!("  {change}");
        }
    }
    if installing && matches!(agent, Agent::Codex) {
        println!("help: open /hooks in Codex to trust the new hooks; Codex skips untrusted ones");
    }
    Ok(ExitCode::SUCCESS)
}

/// Never blocks the agent: any problem goes to stderr and the exit code stays 0.
fn hook(event: HookEvent) -> Result<ExitCode> {
    let mut input = String::new();
    if let Err(err) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("hotword: could not read hook input: {err}");
        return Ok(ExitCode::SUCCESS);
    }
    let payload: serde_json::Value = match serde_json::from_str(&input) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("hotword: hook input is not JSON: {err}");
            return Ok(ExitCode::SUCCESS);
        }
    };
    let cwd = payload["cwd"]
        .as_str()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let store = store_for(&cwd);
    let all = match store.load_all() {
        Ok(all) => all,
        Err(err) => {
            eprintln!("hotword: {err:#}");
            return Ok(ExitCode::SUCCESS);
        }
    };
    let prompt = payload["prompt"].as_str().unwrap_or_default();
    let (event_name, firing): (&str, Vec<&Workflow>) = match event {
        HookEvent::Prompt => (
            "UserPromptSubmit",
            all.iter()
                .map(|l| &l.workflow)
                .filter(|w| w.matches(prompt))
                .collect(),
        ),
        HookEvent::SessionStart => (
            "SessionStart",
            all.iter()
                .map(|l| &l.workflow)
                .filter(|w| w.fires_on(&Event::SessionStart))
                .collect(),
        ),
    };
    if firing.is_empty() {
        return Ok(ExitCode::SUCCESS);
    }
    let reports: Vec<Report> = firing
        .iter()
        .map(|w| runner::run_for_prompt(w, &cwd, prompt))
        .collect();
    let context = reports
        .iter()
        .map(|r| render_text(r, false))
        .collect::<Vec<_>>()
        .join("\n");
    println!("{}", hook_json(event_name, &context));
    Ok(ExitCode::SUCCESS)
}
