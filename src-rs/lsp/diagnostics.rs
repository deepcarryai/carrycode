use crate::lsp::protocol::{Diagnostic, DiagnosticSeverity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    pub errors: usize,
    pub warnings: usize,
    pub hints: usize,
    pub information: usize,
    pub items: Vec<FormattedDiagnostic>,
}

impl fmt::Display for DiagnosticSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut output = String::new();

        // Limit to 10 items per section
        let max_items = 10;
        let file_diagnostics: Vec<_> = self.items.iter().take(max_items).collect();
        let remaining = self.items.len().saturating_sub(max_items);

        if !file_diagnostics.is_empty() {
            output.push_str("\n<file_diagnostics>\n");
            for diag in file_diagnostics {
                let source_info = diag
                    .source
                    .as_ref()
                    .map(|s| format!("[{}]", s))
                    .unwrap_or_default();
                let code_info = diag
                    .code
                    .as_ref()
                    .map(|c| format!("[{}]", c))
                    .unwrap_or_default();
                output.push_str(&format!(
                    "{}: {}:{}:{} {}{} {}\n",
                    diag.severity,
                    diag.file,
                    diag.line,
                    diag.column,
                    source_info,
                    code_info,
                    diag.message
                ));
            }
            if remaining > 0 {
                output.push_str(&format!("... and {} more diagnostics\n", remaining));
            }
            output.push_str("</file_diagnostics>\n");
        }

        output.push_str("\n<diagnostic_summary>\n");
        output.push_str(&format!(
            "Total: {} errors, {} warnings, {} hints\n",
            self.errors, self.warnings, self.hints
        ));
        output.push_str("</diagnostic_summary>\n");

        write!(f, "{}", output)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattedDiagnostic {
    pub severity: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

pub fn format_diagnostics(diagnostics_map: HashMap<String, Vec<Diagnostic>>) -> DiagnosticSummary {
    let mut errors = 0;
    let mut warnings = 0;
    let mut hints = 0;
    let mut information = 0;
    let mut items = Vec::new();

    for (uri, diagnostics) in diagnostics_map {
        let file_path = uri.trim_start_matches("file://");

        for diag in diagnostics {
            let severity_str = match diag.severity {
                Some(DiagnosticSeverity::Error) => {
                    errors += 1;
                    "Error"
                }
                Some(DiagnosticSeverity::Warning) => {
                    warnings += 1;
                    "Warning"
                }
                Some(DiagnosticSeverity::Hint) => {
                    hints += 1;
                    "Hint"
                }
                Some(DiagnosticSeverity::Information) => {
                    information += 1;
                    "Information"
                }
                None => {
                    information += 1;
                    "Information"
                }
            };

            items.push(FormattedDiagnostic {
                severity: severity_str.to_string(),
                file: file_path.to_string(),
                line: diag.range.start.line + 1, // LSP is 0-indexed
                column: diag.range.start.character + 1,
                message: diag.message.clone(),
                source: diag.source.clone(),
                code: diag
                    .code
                    .as_ref()
                    .and_then(|c| c.as_str().map(String::from)),
            });
        }
    }

    // Sort by severity (errors first) then by file
    items.sort_by(|a, b| {
        let severity_order = |s: &str| match s {
            "Error" => 0,
            "Warning" => 1,
            "Information" => 2,
            "Hint" => 3,
            _ => 4,
        };

        severity_order(&a.severity)
            .cmp(&severity_order(&b.severity))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

    DiagnosticSummary {
        errors,
        warnings,
        hints,
        information,
        items,
    }
}
