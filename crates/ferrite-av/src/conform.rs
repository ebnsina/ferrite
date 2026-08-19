//! Standards conformance against DASH-IF.
//!
//! Producing something that plays is not the same as producing something
//! correct. Players are forgiving until a specific device is not.
//!
//! A subprocess-free HTTP client: the validator runs as a container, so this
//! posts a URL and reads the verdict. It never runs in the worker pipeline.

use crate::error::{AvError, Result};
use serde::{Deserialize, Serialize};

/// Where the validator listens.
pub const DEFAULT_ENDPOINT: &str = "http://localhost:8088";

/// How bad a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Breaks the spec. A device will eventually refuse this.
    Error,
    /// Legal but questionable.
    Warning,
    /// Informational only.
    Info,
}

/// One thing the validator said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// How bad.
    pub severity: Severity,
    /// Which test produced it.
    pub section: String,
    /// What it said.
    pub message: String,
}

/// What the validator concluded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// What was checked.
    pub manifest: String,
    /// The validator's own top-level verdict.
    pub passed: bool,
    /// How many checks actually ran. Zero means the validator never reached
    /// the manifest, which it still reports as a pass.
    pub tests_run: usize,
    /// Everything reported, worst first.
    pub findings: Vec<Finding>,
}

impl Verdict {
    /// Whether anything breaks the spec. Warnings do not block.
    ///
    /// A run with no checks in it is not a pass. The validator answers PASS for
    /// a manifest it could not fetch, which is the most dangerous thing it does.
    pub fn is_conformant(&self) -> bool {
        self.passed
            && self.tests_run > 0
            && !self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// Findings at or above `severity`.
    pub fn at_least(&self, severity: Severity) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.severity <= severity)
    }
}

/// Ask the validator about `manifest_url`, which it fetches itself.
///
/// `segments` turns on the deeper check that reads every box. It is much
/// slower, and it is where join bugs show up.
pub fn check(endpoint: &str, manifest_url: &str, segments: bool) -> Result<Verdict> {
    let query = format!(
        "{}/Utils/Process_cli.php?url={}{}",
        endpoint.trim_end_matches('/'),
        manifest_url,
        if segments { "&segments=true" } else { "" },
    );

    let output = std::process::Command::new("curl")
        .args(["-sS", "--max-time", "600", &query])
        .output()
        .map_err(|e| AvError::CodecUnavailable {
            codec: "conformance".into(),
            reason: format!("cannot run curl: {e}"),
        })?;

    if !output.status.success() {
        return Err(AvError::InvalidSpec(format!(
            "conformance request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse(manifest_url, &String::from_utf8_lossy(&output.stdout))
}

/// Read a DASH-IF report into a verdict.
///
/// Split out so the shape is testable without a validator running: the report
/// layout is the part that breaks when they cut a release.
pub fn parse(manifest: &str, report: &str) -> Result<Verdict> {
    let json: serde_json::Value = serde_json::from_str(report)
        .map_err(|e| AvError::InvalidSpec(format!("conformance report is not JSON: {e}")))?;

    let passed = json.get("verdict").and_then(|v| v.as_str()) == Some("PASS");
    let mut findings = Vec::new();
    let mut tests_run = 0;
    collect(&json, &mut findings, &mut tests_run);

    findings.sort_by(|a, b| a.severity.cmp(&b.severity).then(a.section.cmp(&b.section)));
    Ok(Verdict {
        manifest: manifest.to_string(),
        passed,
        tests_run,
        findings,
    })
}

/// Walk the report for `test` arrays, which is where the verdicts live.
fn collect(value: &serde_json::Value, out: &mut Vec<Finding>, tests_run: &mut usize) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if key == "test"
                    && let Some(tests) = child.as_array()
                {
                    *tests_run += tests.len();
                    for test in tests {
                        read_test(test, out);
                    }
                    continue;
                }
                collect(child, out, tests_run);
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|i| collect(i, out, tests_run)),
        _ => {}
    }
}

/// One test entry: a state, and messages marked pass, warn or fail.
fn read_test(test: &serde_json::Value, out: &mut Vec<Finding>) {
    if test.get("state").and_then(|s| s.as_str()) == Some("PASS") {
        return;
    }

    let section = [
        test.get("spec").and_then(|v| v.as_str()).unwrap_or(""),
        test.get("section").and_then(|v| v.as_str()).unwrap_or(""),
        test.get("test").and_then(|v| v.as_str()).unwrap_or(""),
    ]
    .iter()
    .filter(|s| !s.is_empty())
    .copied()
    .collect::<Vec<_>>()
    .join(" / ");

    let Some(messages) = test.get("messages").and_then(|m| m.as_array()) else {
        out.push(Finding {
            severity: Severity::Error,
            section,
            message: "failed".into(),
        });
        return;
    };

    for message in messages.iter().filter_map(|m| m.as_str()) {
        // The report marks each line: cross failed, bang warned, tick passed.
        let (severity, text) = match message.chars().next() {
            Some('\u{2717}') => (Severity::Error, message[3..].trim()),
            Some('!') => (Severity::Warning, message[1..].trim()),
            Some('\u{2713}') => continue,
            _ => (Severity::Error, message.trim()),
        };
        if text.is_empty() {
            continue;
        }
        out.push(Finding {
            severity,
            section: section.clone(),
            message: text.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trimmed from a real run against our own packaged output.
    const PASSING: &str = r#"{
      "parse_segments": false,
      "source": "http://localhost/asset/cmaf/manifest.mpd",
      "entries": {
        "Schematron": {
          "verdict": "PASS",
          "MPD": {
            "verdict": "PASS",
            "test": [{
              "spec": "MPEG-DASH", "section": "Commmon", "test": "Schematron Validation",
              "messages": ["\u2713 XLink resolving succesful", "\u2713 MPD validation succesful"],
              "state": "PASS"
            }]
          }
        }
      },
      "verdict": "PASS"
    }"#;

    const FAILING: &str = r#"{
      "parse_segments": true,
      "entries": {
        "Segments": {
          "test": [{
            "spec": "Segment Validation", "section": "Segment Validation",
            "test": "Segment validator output should not contain errors",
            "messages": [
              "\u2717 'tkhd' trackWidth must be set to one of (0, (320L << 16))",
              "! WARNING: unknown meta atom 'ID32'",
              "\u2713 something fine"
            ],
            "state": "FAIL"
          }]
        }
      },
      "verdict": "FAIL"
    }"#;

    #[test]
    fn a_passing_run_reports_nothing() {
        let v = parse("manifest.mpd", PASSING).unwrap();
        assert!(v.passed);
        assert!(v.is_conformant());
        assert!(v.findings.is_empty(), "{:?}", v.findings);
    }

    #[test]
    fn failures_are_read_with_the_test_that_produced_them() {
        let v = parse("manifest.mpd", FAILING).unwrap();
        assert!(!v.is_conformant());

        let error = &v.findings[0];
        assert_eq!(error.severity, Severity::Error);
        assert!(error.section.contains("Segment Validation"));
        assert!(error.message.contains("trackWidth"), "{}", error.message);
        assert!(
            !error.message.starts_with('\u{2717}'),
            "the marker was not stripped"
        );
    }

    #[test]
    fn a_warning_does_not_block_but_is_still_reported() {
        let v = parse("manifest.mpd", FAILING).unwrap();
        let warnings: Vec<_> = v
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .collect();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("ID32"));
    }

    #[test]
    fn passing_lines_inside_a_failing_test_are_not_findings() {
        let v = parse("manifest.mpd", FAILING).unwrap();
        assert!(
            !v.findings
                .iter()
                .any(|f| f.message.contains("something fine"))
        );
        assert_eq!(v.findings.len(), 2);
    }

    #[test]
    fn findings_come_back_worst_first() {
        let v = parse("manifest.mpd", FAILING).unwrap();
        let order: Vec<Severity> = v.findings.iter().map(|f| f.severity).collect();
        assert_eq!(order, [Severity::Error, Severity::Warning]);
    }

    #[test]
    fn severities_can_be_filtered() {
        let v = parse("manifest.mpd", FAILING).unwrap();
        assert_eq!(v.at_least(Severity::Error).count(), 1);
        assert_eq!(v.at_least(Severity::Warning).count(), 2);
    }

    #[test]
    fn a_run_with_no_checks_in_it_is_not_a_pass() {
        // The validator answers PASS for a manifest it could not fetch. That is
        // the most dangerous thing it does, and it looks exactly like success.
        let unreachable = r#"{"verdict": "PASS", "entries": [], "source": "http://gone/m.mpd"}"#;
        let v = parse("m.mpd", unreachable).unwrap();
        assert!(v.passed, "the tool said PASS");
        assert_eq!(v.tests_run, 0);
        assert!(
            !v.is_conformant(),
            "an empty run was reported as conformant"
        );
    }

    #[test]
    fn a_real_pass_counts_the_checks_that_ran() {
        let v = parse("manifest.mpd", PASSING).unwrap();
        assert!(v.tests_run > 0);
        assert!(v.is_conformant());
    }

    #[test]
    fn a_top_level_fail_is_not_conformant_even_with_no_messages() {
        let bare = r#"{"verdict": "FAIL", "entries": {}}"#;
        assert!(!parse("m.mpd", bare).unwrap().is_conformant());
    }

    #[test]
    fn html_instead_of_json_is_a_clear_error() {
        // The HLS endpoint returns a page, not a report; say so rather than
        // reporting a clean run.
        let err = parse("m.m3u8", "<!DOCTYPE html>").unwrap_err().to_string();
        assert!(err.contains("not JSON"), "{err}");
    }
}
