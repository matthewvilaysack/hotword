//! Workflow definitions: the TOML schema, validation, trigger matching, and the
//! two places workflows live (a repo's `.hotword/` and the user's config dir).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const PROJECT_DIR: &str = ".hotword";
pub const DEFAULT_TIMEOUT: u64 = 30;
pub const DEFAULT_MAX_LINES: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Event {
    SessionStart,
}

impl Event {
    pub fn as_str(&self) -> &'static str {
        match self {
            Event::SessionStart => "session-start",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    pub name: String,
    pub run: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Binaries that must be on PATH for the step to run. Defaults to the first
    /// word of `run` unless that word is a shell builtin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on: Vec<Event>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_max_lines")]
    pub max_lines: usize,
    #[serde(default)]
    pub steps: Vec<Step>,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT
}

fn default_max_lines() -> usize {
    DEFAULT_MAX_LINES
}

impl Workflow {
    pub fn from_toml(text: &str) -> Result<Workflow> {
        let wf: Workflow = toml::from_str(text)?;
        wf.validate()?;
        Ok(wf)
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("workflow serializes")
    }

    pub fn validate(&self) -> Result<()> {
        if !is_slug(&self.name) {
            bail!(
                "name {:?} must be lowercase letters, digits, and hyphens (like apple-status)",
                self.name
            );
        }
        if self.steps.is_empty() {
            bail!("workflow {} needs at least one step", self.name);
        }
        if self.triggers.is_empty() && self.on.is_empty() {
            bail!(
                "workflow {} needs a trigger phrase or an `on` event",
                self.name
            );
        }
        for step in &self.steps {
            if step.run.trim().is_empty() {
                bail!(
                    "step {:?} in {} has an empty run command",
                    step.name,
                    self.name
                );
            }
        }
        Ok(())
    }

    /// True when any trigger phrase appears in the prompt, ignoring case and
    /// runs of whitespace.
    pub fn matches(&self, prompt: &str) -> bool {
        self.triggers
            .iter()
            .any(|t| self.matches_trigger(t, prompt))
    }

    pub fn matches_trigger(&self, trigger: &str, prompt: &str) -> bool {
        normalize(prompt).contains(&normalize(trigger))
    }

    pub fn fires_on(&self, event: &Event) -> bool {
        self.on.contains(event)
    }
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn is_slug(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Project,
    User,
}

impl Source {
    pub fn as_str(&self) -> &'static str {
        match self {
            Source::Project => "project",
            Source::User => "user",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub workflow: Workflow,
    pub path: PathBuf,
    pub source: Source,
}

/// Where workflows are read from and written to. Project workflows shadow user
/// workflows with the same name.
#[derive(Debug, Clone)]
pub struct Store {
    pub user_dir: PathBuf,
    pub project_dir: Option<PathBuf>,
}

impl Store {
    /// Walks up from `cwd` looking for a `.hotword/` directory; the user dir is
    /// `$HOME/.config/hotword`.
    pub fn discover(cwd: &Path, home: &Path) -> Store {
        let project_dir = cwd
            .ancestors()
            .map(|dir| dir.join(PROJECT_DIR))
            .find(|candidate| candidate.is_dir());
        Store {
            user_dir: home.join(".config").join("hotword"),
            project_dir,
        }
    }

    pub fn dir_for(&self, source: Source) -> Result<&Path> {
        match source {
            Source::User => Ok(&self.user_dir),
            Source::Project => self.project_dir.as_deref().ok_or_else(|| {
                anyhow!("no {PROJECT_DIR}/ directory here; run `mkdir {PROJECT_DIR}` at the repo root first")
            }),
        }
    }

    pub fn load_all(&self) -> Result<Vec<Loaded>> {
        let mut found: Vec<Loaded> = Vec::new();
        for (source, dir) in self.dirs() {
            for loaded in load_dir(&dir, source)? {
                if !found
                    .iter()
                    .any(|l| l.workflow.name == loaded.workflow.name)
                {
                    found.push(loaded);
                }
            }
        }
        found.sort_by(|a, b| a.workflow.name.cmp(&b.workflow.name));
        Ok(found)
    }

    pub fn find(&self, name: &str) -> Result<Option<Loaded>> {
        Ok(self
            .load_all()?
            .into_iter()
            .find(|l| l.workflow.name == name))
    }

    pub fn save(&self, workflow: &Workflow, source: Source) -> Result<PathBuf> {
        workflow.validate()?;
        let dir = self.dir_for(source)?;
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let path = dir.join(format!("{}.toml", workflow.name));
        fs::write(&path, workflow.to_toml())
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(path)
    }

    pub fn remove(&self, name: &str) -> Result<PathBuf> {
        let loaded = self
            .find(name)?
            .ok_or_else(|| anyhow!("no workflow named {name}; run `hotword` to list them"))?;
        fs::remove_file(&loaded.path)
            .with_context(|| format!("removing {}", loaded.path.display()))?;
        Ok(loaded.path)
    }

    /// Project first so it shadows the user dir.
    fn dirs(&self) -> Vec<(Source, PathBuf)> {
        let mut dirs = Vec::new();
        if let Some(project) = &self.project_dir {
            dirs.push((Source::Project, project.clone()));
        }
        dirs.push((Source::User, self.user_dir.clone()));
        dirs
    }
}

fn load_dir(dir: &Path, source: Source) -> Result<Vec<Loaded>> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let workflow =
            Workflow::from_toml(&text).with_context(|| format!("in {}", path.display()))?;
        out.push(Loaded {
            workflow,
            path,
            source,
        });
    }
    Ok(out)
}
