use std::sync::Arc;

use camino::Utf8PathBuf;
use normfix_core::{Diagnostic, DiagnosticSource, FixRecord, Severity, TextRange, TextSize};

use crate::FileReport;

pub(super) fn diagnostic() -> Diagnostic {
    Diagnostic {
        rule_id: "TOO_MANY_LINES".to_owned(),
        path: Utf8PathBuf::from("src/main.c"),
        range: TextRange::new(TextSize::new(5), TextSize::new(9)).expect("range"),
        severity: Severity::Warning,
        message: "Function exceeds the 25-line limit".to_owned(),
        source: DiagnosticSource::NativeNorm41,
        notes: vec!["main() has 30 body lines".to_owned()],
        help: Some("Extract one coherent responsibility into a static helper.".to_owned()),
        localized: None,
    }
}

pub(super) fn file() -> FileReport {
    let source: Arc<str> = Arc::from("int\tmain(void)\n{\n\treturn (0);\n}\n");
    FileReport {
        budget: Vec::new(),
        path: Utf8PathBuf::from("src/main.c"),
        changed: true,
        written: false,
        backup: None,
        failure: None,
        fixes: vec![FixRecord {
            rule_id: "MIXED_SPACE_TAB".to_owned(),
            description: "normalized indentation".to_owned(),
            line: Some(3),
            count: 1,
        }],
        before: Vec::new(),
        after: vec![diagnostic()],
        original: Some(Arc::clone(&source)),
        fixed: Some(source),
    }
}
