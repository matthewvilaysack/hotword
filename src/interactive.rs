//! The guided `hotword add` for a person at a terminal. Everything here is also
//! reachable with flags, so agents never land in a prompt.

use anyhow::Result;
use inquire::{Confirm, Text};

use crate::cli::parse_step;
use crate::workflow::{Event, Step, Workflow, DEFAULT_MAX_LINES};

pub fn build(name: &str, description: Option<String>, timeout: u64) -> Result<Workflow> {
    let description = match description {
        Some(d) => Some(d),
        None => {
            let text = Text::new("One line on what this workflow tells you:").prompt()?;
            (!text.trim().is_empty()).then_some(text)
        }
    };
    let mut triggers = Vec::new();
    loop {
        let label = if triggers.is_empty() {
            "Phrase that fires it when typed to the agent (blank for none):"
        } else {
            "Another phrase (blank to stop):"
        };
        let phrase = Text::new(label).prompt()?;
        if phrase.trim().is_empty() {
            break;
        }
        triggers.push(phrase.trim().to_string());
    }
    let on_start = Confirm::new("Also run at session start?")
        .with_default(false)
        .prompt()?;
    let mut steps: Vec<Step> = Vec::new();
    loop {
        let label = if steps.is_empty() {
            "First step, as \"name: command\" or just a command:"
        } else {
            "Next step (blank to finish):"
        };
        let text = Text::new(label).prompt()?;
        if text.trim().is_empty() {
            if steps.is_empty() {
                continue;
            }
            break;
        }
        steps.push(parse_step(&text));
    }
    let workflow = Workflow {
        name: name.to_string(),
        description,
        triggers,
        on: if on_start {
            vec![Event::SessionStart]
        } else {
            Vec::new()
        },
        timeout,
        max_lines: DEFAULT_MAX_LINES,
        steps,
    };
    workflow.validate()?;
    Ok(workflow)
}
