use crate::model::RunReport;

impl RunReport {
    /// Serializes stable pretty JSON.
    ///
    /// # Errors
    ///
    /// Returns a serialization error only if the report schema becomes
    /// internally inconsistent.
    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self).map(|mut json| {
            json.push('\n');
            json
        })
    }
}
