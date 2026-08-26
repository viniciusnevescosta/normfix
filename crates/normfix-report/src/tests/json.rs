use std::time::Duration;

use camino::Utf8PathBuf;

use super::common::file;
use crate::{ReportIdentity, ReportMode, RunReport};

#[test]
fn json_is_versioned_sorted_and_does_not_include_source_buffers() {
    let report = RunReport::new(
        "0.4.0",
        ReportMode::Diff,
        ReportIdentity::default(),
        vec!["z".to_owned(), "a".to_owned()],
        vec![Utf8PathBuf::from("z.bin"), Utf8PathBuf::from("a.bin")],
        vec![file()],
        Duration::ZERO,
    );
    let json = report.to_pretty_json().expect("JSON");

    assert_eq!(report.summary.files, 1);
    assert_eq!(report.summary.changed, 1);
    assert_eq!(report.summary.written, 0);
    assert_eq!(report.summary.fixes, 1);
    assert_eq!(report.summary.remaining, 1);
    assert_eq!(report.summary.advisories, 0);
    assert_eq!(report.summary.failed, 0);
    assert_eq!(report.summary.unexpected_files, 2);
    assert!(json.contains("\"schema_version\": 2"));
    assert!(json.contains("\"mode\": \"diff\""));
    assert!(!json.contains("\"original\""));
    assert!(!json.contains("\"fixed\""));
    assert!(json.find("\"a\"").expect("a") < json.find("\"z\"").expect("z"));
    assert_eq!(report.exit_code(), 2);
}

#[test]
fn report_schema_one_without_evaluation_still_deserializes() {
    let fixture = include_str!("../../tests/fixtures/report-schema-v1.json");
    let decoded: RunReport = serde_json::from_str(fixture).expect("schema-one report");

    assert_eq!(decoded.schema_version, 1);
    assert!(decoded.evaluation.is_none());
    assert_eq!(decoded.tool_version, "1.0.0-rc.0");
}
