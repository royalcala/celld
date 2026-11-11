// Copyright 2026 Deno Land Inc. Apache-2.0 license.

//! `celld log` — inspect the node-log sessions a fleet bucket holds.
//!
//! A takeover can be refused while recovery cannot decide an ensemble
//! member's fate, and today that answer only exists in the node's own log.
//! This command reads the bucket directly: no running node, no writes, and no
//! alternate seal path. It reports each open or recovering session and every
//! member's lease so an operator can see which member is undecided and why.
//!
//! It deliberately has no `prune`. Recovery already declares the bounded loss
//! and drains the retained bundle; a second, manual seal path would have to
//! repeat those guarantees to be safe.

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use std::borrow::Cow;

use crate::cli_options::FleetFlags;
use crate::cli_options::FLEET_HELP;
use crate::cli_output::align;
use crate::cli_output::Format;
use crate::cli_output::Output;
use crate::cli_output::Record;
use crate::node_log;
use crate::note;
use crate::ownership_store::now_ms;

#[derive(Default)]
struct StatusOptions {
    fleet: FleetFlags,
    json: bool,
}

struct StatusRow {
    session: String,
    state: String,
    epoch: u64,
    fleet_acks: bool,
    member: String,
    lease: &'static str,
    detail: String,
}

impl Record for StatusRow {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "session": self.session,
            "state": self.state,
            "epoch": self.epoch,
            "may_have_fleet_acks": self.fleet_acks,
            "member": self.member,
            "lease": self.lease,
            "detail": self.detail,
        })
    }

    fn text(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{}  {}  {}  {}  {}  {}",
            self.session, self.state, self.epoch, self.member, self.lease, self.detail
        ))
    }
}

pub async fn run(arguments: Vec<String>) -> anyhow::Result<()> {
    let mut arguments = arguments.into_iter();
    match arguments.next().as_deref() {
        Some("status") => run_status(arguments.collect()).await,
        None | Some("-h") | Some("--help") | Some("help") => {
            Output::new(Format::Text).help(&help_text())
        }
        Some(other) => bail!("unknown log command: {other}; use celld log status"),
    }
}

fn status_options(arguments: Vec<String>) -> anyhow::Result<Option<StatusOptions>> {
    let mut options = StatusOptions::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        let mut value = |option: &str| {
            arguments
                .next()
                .ok_or_else(|| anyhow!("{option} requires a value"))
        };
        match argument.as_str() {
            "-h" | "--help" => return Ok(None),
            "--json" => options.json = true,
            other => {
                if options.fleet.consume(other, &mut value)? {
                    continue;
                }
                bail!(
                    "unknown log status option: {other}; run `celld log status --help` for usage"
                );
            }
        }
    }
    Ok(Some(options))
}

async fn run_status(arguments: Vec<String>) -> anyhow::Result<()> {
    let Some(options) = status_options(arguments)? else {
        return Output::new(Format::Text).help(&help_text());
    };
    let storage = options.fleet.resolve("celld log status")?;
    let bucket = storage.open().await?;
    let now = now_ms();
    // Recovery uses 3x the node's lease TTL with a 20 s floor. The CLI reads
    // the same variable because the bucket does not carry the TTL; an operator
    // who configured a different one sees the estimate for that TTL.
    let ttl_ms = crate::env_vars::positive_or("CELLD_TTL_MS", 10_000u64).unwrap_or(10_000);
    let grace_ms = ttl_ms.saturating_mul(3).max(20_000);

    let mut objects = bucket.list("nodes/").await.context("list node leases")?;
    objects.sort_by(|left, right| left.location.as_ref().cmp(right.location.as_ref()));

    let mut rows = Vec::new();
    for object in objects {
        let key = object.location.as_ref();
        let Some(file) = key.strip_prefix("nodes/") else {
            continue;
        };
        let Some(node) = file.strip_suffix(".json") else {
            continue;
        };
        let lease = match node_log::read_lease_wire(&bucket, node).await {
            Ok(lease) => lease,
            Err(error) => {
                note!("skipped {key}: {error:#}");
                continue;
            }
        };
        let Some(lease) = lease else { continue };
        let Some(log) = lease.log.as_ref() else {
            continue;
        };
        // A sealed record is complete: no takeover waits on it.
        if log.state == "sealed" {
            continue;
        }
        let session = format!("{}/{}", lease.node, lease.generation);
        if log.ensemble.is_empty() {
            rows.push(StatusRow {
                session,
                state: log.state.clone(),
                epoch: log.epoch,
                fleet_acks: log.may_have_fleet_acks,
                member: "-".to_string(),
                lease: "no members",
                detail: String::new(),
            });
            continue;
        }
        for member in &log.ensemble {
            let (lease_state, detail) = match node_log::read_lease_wire(&bucket, member).await {
                Ok(Some(record)) => classify_member(Some(record.expires_ms), None, now, grace_ms),
                Ok(None) => {
                    let first =
                        node_log::absent_member_first_seen(&bucket, &session, log.epoch, member)
                            .await
                            .unwrap_or(None);
                    classify_member(None, first, now, grace_ms)
                }
                Err(error) => ("unreadable", format!("{error:#}")),
            };
            rows.push(StatusRow {
                session: session.clone(),
                state: log.state.clone(),
                epoch: log.epoch,
                fleet_acks: log.may_have_fleet_acks,
                member: member.clone(),
                lease: lease_state,
                detail,
            });
        }
    }

    let mut out = Output::new(if options.json {
        Format::Json
    } else {
        Format::Text
    });
    if options.json {
        for row in &rows {
            out.row(row)?;
        }
        return out.finish();
    }
    if rows.is_empty() {
        return out.line(format_args!("no open or recovering node-log sessions"));
    }
    let columns = vec![
        "SESSION".to_string(),
        "STATE".to_string(),
        "EPOCH".to_string(),
        "FLEET ACKS".to_string(),
        "MEMBER".to_string(),
        "LEASE".to_string(),
        "DETAIL".to_string(),
    ];
    let cells = rows
        .iter()
        .map(|row| {
            vec![
                row.session.clone(),
                row.state.clone(),
                row.epoch.to_string(),
                if row.fleet_acks { "yes" } else { "no" }.to_string(),
                row.member.clone(),
                row.lease.to_string(),
                row.detail.clone(),
            ]
        })
        .collect::<Vec<_>>();
    let (header, rule, lines) = align(&columns, &cells);
    let mut text = format!("{header}\n{rule}");
    for line in lines {
        text.push('\n');
        text.push_str(&line);
    }
    out.line(format_args!("{text}"))
}

/// The fate of one ensemble member, as recovery would see it.
///
/// `expires_ms` is the member's lease expiry when its record exists. A missing
/// record is different: the dead-node GC deletes a dead record that carries no
/// folded log, so absence has no timestamp of its own. Recovery dates that
/// absence in a marker; `absent_first_seen_ms` is that date.
fn classify_member(
    expires_ms: Option<u64>,
    absent_first_seen_ms: Option<u64>,
    now_ms: u64,
    grace_ms: u64,
) -> (&'static str, String) {
    match expires_ms {
        Some(expires) if expires > now_ms => {
            ("live", format!("expires in {}s", (expires - now_ms) / 1000))
        }
        Some(expires) => (
            "expired",
            format!("expired {}s ago", now_ms.saturating_sub(expires) / 1000),
        ),
        None => match absent_first_seen_ms {
            None => (
                "absent",
                "no absence recorded yet; the next recovery pass dates it".to_string(),
            ),
            Some(first) => {
                let age = now_ms.saturating_sub(first);
                if age >= grace_ms {
                    (
                        "absent",
                        format!(
                            "absent {}s; grace elapsed, seals on the next recovery pass",
                            age / 1000
                        ),
                    )
                } else {
                    (
                        "absent",
                        format!(
                            "absent {}s; seals after the grace ({}s more)",
                            age / 1000,
                            (grace_ms - age) / 1000
                        ),
                    )
                }
            }
        },
    }
}

fn help_text() -> String {
    format!(
        r#"celld log — inspect the node-log sessions a bucket holds

USAGE:
  celld log status --bucket [s3://|gs://|az://]NAME[/PREFIX] [OPTIONS]

STATUS:
  Lists every open or recovering node-log session and each ensemble member:
  live, expired, absent (with the recorded absence age) or unreadable. An
  absent member blocks recovery until the 3x TTL grace (minimum 20 s) has
  passed since the first sighting; the next recovery pass then declares the
  bounded loss, drains the retained bundle and seals. This command reads the
  bucket and writes nothing.

OPTIONS:
{FLEET_HELP}
  --json               Print one JSON object per member
  -h, --help           Show this help"#
    )
}

#[cfg(test)]
mod tests {
    use super::classify_member;

    const NOW: u64 = 1_000_000;
    const GRACE: u64 = 20_000;

    #[test]
    fn a_live_lease_is_not_undecided() {
        assert_eq!(
            classify_member(Some(NOW + 5_000), None, NOW, GRACE),
            ("live", "expires in 5s".to_string())
        );
    }

    #[test]
    fn an_expired_lease_reports_its_age() {
        assert_eq!(
            classify_member(Some(NOW - 7_000), None, NOW, GRACE),
            ("expired", "expired 7s ago".to_string())
        );
    }

    #[test]
    fn an_absent_member_without_a_marker_is_undated() {
        let (state, detail) = classify_member(None, None, NOW, GRACE);
        assert_eq!(state, "absent");
        assert!(detail.contains("no absence recorded"));
    }

    #[test]
    fn an_absent_member_inside_the_grace_reports_the_remaining_wait() {
        let (state, detail) = classify_member(None, Some(NOW - 5_000), NOW, GRACE);
        assert_eq!(state, "absent");
        assert!(detail.contains("absent 5s"));
        assert!(detail.contains("15s more"));
    }

    #[test]
    fn an_absent_member_past_the_grace_will_seal_on_the_next_pass() {
        let (state, detail) = classify_member(None, Some(NOW - 21_000), NOW, GRACE);
        assert_eq!(state, "absent");
        assert!(detail.contains("grace elapsed"));
    }
}
