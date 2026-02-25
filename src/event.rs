use anyhow::{Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;

#[derive(Debug, Deserialize)]
struct PushEventPayload {
    before: String,
}

pub fn push_range_from_env() -> Result<Option<(String, String)>> {
    let event_name = env::var("GITHUB_EVENT_NAME").unwrap_or_default();
    if event_name != "push" {
        return Ok(None);
    }

    let after = env::var("GITHUB_SHA").context("missing GITHUB_SHA")?;
    let event_path = env::var("GITHUB_EVENT_PATH").context("missing GITHUB_EVENT_PATH")?;

    let raw =
        fs::read_to_string(&event_path).with_context(|| format!("failed reading {event_path}"))?;
    let payload = serde_json::from_str::<PushEventPayload>(&raw)
        .with_context(|| format!("failed parsing github event payload {event_path}"))?;

    if payload.before == "0000000000000000000000000000000000000000" {
        return Ok(None);
    }

    Ok(Some((payload.before, after)))
}
