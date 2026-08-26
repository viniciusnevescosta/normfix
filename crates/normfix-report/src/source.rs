use std::collections::BTreeMap;

use camino::Utf8Path;

use crate::model::FileReport;

/// Builds a map used by callers that need direct source lookup by path.
#[must_use]
pub fn source_map(files: &[FileReport]) -> BTreeMap<&Utf8Path, &str> {
    files
        .iter()
        .filter_map(|file| {
            file.fixed
                .as_deref()
                .or(file.original.as_deref())
                .map(|source| (file.path.as_path(), source))
        })
        .collect()
}
