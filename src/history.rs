//! Optional run history in a Dolt repository: every run is a commit, so the
//! team can `dolt diff`, `dolt log`, push it to a shared remote, and ask SQL
//! questions about what a status check said last week. Nothing here runs
//! unless `hotword history init` was run once and `dolt` is on PATH.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::runner::{on_path, Report};

pub const DIR_NAME: &str = "history";

#[derive(Debug, Clone)]
pub struct History {
    pub dir: PathBuf,
    home: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Run {
    pub id: String,
    pub workflow: String,
    pub started: String,
    pub host: String,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(deserialize_with = "int")]
    pub ok: i64,
    #[serde(deserialize_with = "int")]
    pub fail: i64,
    #[serde(deserialize_with = "int")]
    pub skip: i64,
    #[serde(deserialize_with = "int")]
    pub timeout: i64,
}

/// One step whose status or output differs between the last two runs.
#[derive(Debug, Clone)]
pub struct Change {
    pub name: String,
    pub before_status: Option<String>,
    pub after_status: Option<String>,
    pub output_changed: bool,
    pub after: String,
}

impl History {
    /// `dir` is where the Dolt repo lives; `home` is where Dolt keeps its
    /// global config (the real home normally, a sandbox in tests).
    pub fn new(dir: PathBuf, home: PathBuf) -> History {
        History { dir, home }
    }

    pub fn under(hotword_home: &Path, home: &Path) -> History {
        History::new(hotword_home.join(DIR_NAME), home.to_path_buf())
    }

    pub fn exists(&self) -> bool {
        self.dir.join(".dolt").is_dir()
    }

    pub fn init(&self) -> Result<()> {
        if !on_path("dolt") {
            bail!("dolt is not on PATH; install it from https://github.com/dolthub/dolt");
        }
        std::fs::create_dir_all(&self.dir)
            .with_context(|| format!("creating {}", self.dir.display()))?;
        if !self.exists() {
            self.dolt(&["init", "--name", "hotword", "--email", "hotword@localhost"])?;
        }
        self.sql(
            "CREATE TABLE IF NOT EXISTS runs (
                id VARCHAR(36) PRIMARY KEY, workflow VARCHAR(100) NOT NULL, started DATETIME NOT NULL,
                host VARCHAR(100) NOT NULL, prompt TEXT, ok INT NOT NULL, fail INT NOT NULL,
                skip INT NOT NULL, timeout INT NOT NULL, INDEX (workflow, started));
             CREATE TABLE IF NOT EXISTS steps (
                run_id VARCHAR(36) NOT NULL, name VARCHAR(100) NOT NULL, status VARCHAR(10) NOT NULL,
                exit_code INT, ms BIGINT NOT NULL, output LONGTEXT NOT NULL, reason TEXT,
                PRIMARY KEY (run_id, name));
             CREATE TABLE IF NOT EXISTS latest (
                workflow VARCHAR(100) NOT NULL, name VARCHAR(100) NOT NULL, status VARCHAR(10) NOT NULL,
                exit_code INT, ms BIGINT NOT NULL, output LONGTEXT NOT NULL, reason TEXT,
                run_id VARCHAR(36) NOT NULL, PRIMARY KEY (workflow, name));",
        )?;
        self.commit("Create the hotword history tables")?;
        Ok(())
    }

    /// Records a run as one commit. A no-op when history was never initialised.
    pub fn record(&self, report: &Report, prompt: Option<&str>) -> Result<()> {
        if !self.exists() || !on_path("dolt") {
            return Ok(());
        }
        let id = run_id();
        let host = hostname();
        let mut sql = format!(
            "INSERT INTO runs VALUES ({}, {}, NOW(), {}, {}, {}, {}, {}, {});",
            q(&id),
            q(&report.workflow),
            q(&host),
            prompt.map_or("NULL".to_string(), q),
            report.count(crate::runner::Status::Ok),
            report.count(crate::runner::Status::Fail),
            report.count(crate::runner::Status::Skip),
            report.count(crate::runner::Status::Timeout),
        );
        sql.push_str(&format!(
            "DELETE FROM latest WHERE workflow = {};",
            q(&report.workflow)
        ));
        for step in &report.steps {
            let exit = step.exit.map_or("NULL".to_string(), |c| c.to_string());
            let reason = step.reason.as_deref().map_or("NULL".to_string(), q);
            sql.push_str(&format!(
                "INSERT INTO steps VALUES ({}, {}, {}, {}, {}, {}, {});",
                q(&id),
                q(&step.name),
                q(step.status.as_str()),
                exit,
                step.ms,
                q(&step.output),
                reason
            ));
            sql.push_str(&format!(
                "INSERT INTO latest VALUES ({}, {}, {}, {}, {}, {}, {}, {});",
                q(&report.workflow),
                q(&step.name),
                q(step.status.as_str()),
                exit,
                step.ms,
                q(&step.output),
                reason,
                q(&id)
            ));
        }
        self.sql(&sql)?;
        self.commit(&format!("Run {}", report.workflow))
    }

    pub fn runs(&self, workflow: Option<&str>, limit: usize) -> Result<Vec<Run>> {
        let filter = workflow.map_or(String::new(), |w| format!("WHERE workflow = {} ", q(w)));
        let rows = self.sql_json(&format!(
            "SELECT id, workflow, started, host, prompt, ok, fail, skip, timeout FROM runs {filter}ORDER BY started DESC, id DESC LIMIT {limit}"
        ))?;
        rows.into_iter()
            .map(|r| serde_json::from_value(Value::Object(r)).context("reading a run row"))
            .collect()
    }

    /// Steps of `workflow` whose status or output differs between the last
    /// two recorded runs, using Dolt's diff of the `latest` table.
    pub fn changes(&self, workflow: &str) -> Result<Vec<Change>> {
        let commits = self.sql_json(&format!(
            "SELECT commit_hash FROM dolt_log WHERE message = {} ORDER BY date DESC LIMIT 2",
            q(&format!("Run {workflow}"))
        ))?;
        if commits.len() < 2 {
            return Ok(Vec::new());
        }
        let newer = commits[0]["commit_hash"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let older = commits[1]["commit_hash"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let rows =
            self.sql_json(&format!(
            "SELECT to_name, from_name, from_status, to_status, from_output, to_output, diff_type
             FROM dolt_diff({}, {}, 'latest') WHERE COALESCE(to_workflow, from_workflow) = {}",
            q(&older), q(&newer), q(workflow)
        ))?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let name = text(&r["to_name"])
                    .or_else(|| text(&r["from_name"]))
                    .unwrap_or_default();
                let before = text(&r["from_output"]).unwrap_or_default();
                let after = text(&r["to_output"]).unwrap_or_default();
                Change {
                    name,
                    before_status: text(&r["from_status"]),
                    after_status: text(&r["to_status"]),
                    output_changed: before != after,
                    after,
                }
            })
            .filter(|c| c.output_changed || c.before_status != c.after_status)
            .collect())
    }

    pub fn sql_json(&self, query: &str) -> Result<Vec<serde_json::Map<String, Value>>> {
        let out = self.dolt(&["sql", "-r", "json", "-q", query])?;
        let value: Value = serde_json::from_str(out.trim())
            .with_context(|| format!("dolt returned non-JSON for: {query}"))?;
        Ok(value["rows"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.as_object().cloned())
            .collect())
    }

    fn sql(&self, query: &str) -> Result<()> {
        self.dolt(&["sql", "-q", query]).map(|_| ())
    }

    fn commit(&self, message: &str) -> Result<()> {
        self.dolt(&["add", "-A"])?;
        let out = Command::new("dolt")
            .args(["commit", "-m", message])
            .current_dir(&self.dir)
            .env("HOME", &self.home)
            .env("DOLT_ROOT_PATH", self.home.join(".dolt"))
            .stdin(Stdio::null())
            .output()
            .context("running dolt commit")?;
        let text = String::from_utf8_lossy(&out.stderr);
        if out.status.success()
            || text.contains("nothing to commit")
            || text.contains("no changes added")
        {
            Ok(())
        } else {
            Err(anyhow!("dolt commit failed: {}", text.trim()))
        }
    }

    fn dolt(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("dolt")
            .args(args)
            .current_dir(&self.dir)
            .env("HOME", &self.home)
            .env("DOLT_ROOT_PATH", self.home.join(".dolt"))
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("running dolt {}", args.join(" ")))?;
        if !out.status.success() {
            bail!(
                "dolt {} failed: {}",
                args.first().unwrap_or(&""),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

fn q(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "''"))
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(String::from)
}

fn int<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<i64, D::Error> {
    let v = Value::deserialize(d)?;
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| serde::de::Error::custom("not an integer"))
}

fn run_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}-{:x}", nanos, std::process::id())
}

fn hostname() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
