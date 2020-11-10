use path_clean::PathClean;
use regex::Regex;
use serde::Deserialize;
use serde_xml_rs::from_reader;
use std::env::current_dir;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
// use tempdir::TempDir;
use tower_lsp::lsp_types::*;
#[cfg(feature = "slang")]
use veridian_slang::slang_compile;
use walkdir::{DirEntry, WalkDir};

#[cfg(feature = "slang")]
pub fn get_diagnostics(uri: Url, files: Vec<Url>, hal: bool) -> PublishDiagnosticsParams {
    if !(cfg!(test) && (uri.to_string().starts_with("file:///test"))) {
        let paths = get_paths(files);
        let diagnostics = {
            if hal {
                match hal_lint(&uri, paths) {
                    Some(diags) => diags,
                    None => Vec::new(),
                }
            } else if cfg!(feature = "slang") {
                parse_report(uri.clone(), slang_compile(paths).unwrap())
            } else {
                Vec::new()
            }
        };
        PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        }
    } else {
        PublishDiagnosticsParams {
            uri,
            diagnostics: Vec::new(),
            version: None,
        }
    }
}

#[cfg(not(feature = "slang"))]
pub fn get_diagnostics(uri: Url, files: Vec<Url>, hal: bool) -> PublishDiagnosticsParams {
    if !(cfg!(test) && (uri.to_string().starts_with("file:///test"))) {
        let paths = get_paths(files);
        let diagnostics = {
            if hal {
                match hal_lint(&uri, paths) {
                    Some(diags) => diags,
                    None => Vec::new(),
                }
            } else {
                Vec::new()
            }
        };
        PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        }
    } else {
        PublishDiagnosticsParams {
            uri,
            diagnostics: Vec::new(),
            version: None,
        }
    }
}

fn get_paths(files: Vec<Url>) -> Vec<PathBuf> {
    // check recursively from working dir for source files
    let mut paths: Vec<PathBuf> = Vec::new();
    let walker = WalkDir::new(".").into_iter();
    for entry in walker.filter_entry(|e| !is_hidden(e)) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let extension = entry.path().extension().unwrap();

            if extension == "sv" || extension == "svh" || extension == "v" || extension == "vh" {
                paths.push(entry.path().to_path_buf());
            }
        }
    }

    // check recursively from opened files for source files
    for file in files {
        if let Ok(path) = file.to_file_path() {
            if !paths.contains(&path) {
                let walker = WalkDir::new(path.parent().unwrap()).into_iter();
                for entry in walker.filter_entry(|e| !is_hidden(e)) {
                    if let Ok(entry) = entry {
                        if entry.file_type().is_file() && entry.path().extension().is_some() {
                            let extension = entry.path().extension().unwrap();

                            if extension == "sv"
                                || extension == "svh"
                                || extension == "v"
                                || extension == "vh"
                            {
                                let entry_path = entry.path().to_path_buf();
                                if !paths.contains(&entry_path) {
                                    paths.push(entry_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    paths
}

pub fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(feature = "slang")]
fn parse_report(uri: Url, report: String) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for line in report.lines() {
        let diag: Vec<&str> = line.splitn(5, ':').collect();
        if absolute_path(diag.get(0).unwrap()).unwrap() == uri.to_file_path().unwrap().as_os_str() {
            let pos = Position::new(
                diag.get(1).unwrap().parse::<u64>().unwrap() - 1,
                diag.get(2).unwrap().parse::<u64>().unwrap() - 1,
            );
            diagnostics.push(Diagnostic::new(
                Range::new(pos, pos),
                slang_severity(diag.get(3).unwrap()),
                None,
                Some("slang".to_owned()),
                (*diag.get(4).unwrap()).to_owned(),
                None,
                None,
            ))
        }
    }
    diagnostics
}

#[cfg(feature = "slang")]
fn slang_severity(severity: &str) -> Option<DiagnosticSeverity> {
    match severity {
        " error" => Some(DiagnosticSeverity::Error),
        " warning" => Some(DiagnosticSeverity::Warning),
        " note" => Some(DiagnosticSeverity::Information),
        _ => None,
    }
}

// convert relative path to absolute
fn absolute_path(path_str: &str) -> io::Result<PathBuf> {
    let path = Path::new(path_str);
    Ok(current_dir().unwrap().join(path).clean())
}

#[derive(Debug, Deserialize)]
enum HalSeverity {
    #[serde(rename = "fatal")]
    Fatal,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warning")]
    Warning,
    #[serde(rename = "note")]
    Note,
    #[serde(rename = "info")]
    Info,
}

impl From<HalSeverity> for DiagnosticSeverity {
    fn from(severity: HalSeverity) -> Self {
        match severity {
            HalSeverity::Fatal => DiagnosticSeverity::Error,
            HalSeverity::Error => DiagnosticSeverity::Error,
            HalSeverity::Warning => DiagnosticSeverity::Warning,
            HalSeverity::Note => DiagnosticSeverity::Information,
            HalSeverity::Info => DiagnosticSeverity::Information,
        }
    }
}

#[derive(Debug, Deserialize)]
struct HalMessage {
    id: String,
    severity: HalSeverity,
    info: String,
    source_line: String,
    file_info: String,
    help: String,
    object: String,
}

#[derive(Debug, Deserialize)]
struct HalMessageFile {
    tool: String,
    version: String,
    timestamp: String,
    #[serde(rename = "message", default)]
    messages: Vec<HalMessage>,
}

// Lint using the Cadence Incisive HDL analysis technology (HAL)
fn hal_lint(uri: &Url, paths: Vec<PathBuf>) -> Option<Vec<Diagnostic>> {
    // get all file paths
    let mut path_strs = Vec::new();
    for path in paths {
        if let Some(path_str) = path.to_str() {
            path_strs.push(path_str.to_string());
        }
    }
    // using temp dir breaks hal
    // let tmp_dir = TempDir::new("veridian").ok()?;
    Command::new("hal")
        .arg("-64BIT")
        .arg("-SV")
        .arg("-NOSTDOUT")
        .arg("-NO_DESIGN_FACTS")
        .arg("-XMLFILE")
        .arg("hal.xml")
        .args(path_strs)
        .spawn()
        .expect("hal call failed");

    // retreive and parse xml report
    let report = fs::read_to_string("hal.xml").ok()?;
    let hal_report: HalMessageFile = from_reader(report.as_bytes()).ok()?;

    let mut diags: Vec<Diagnostic> = Vec::new();
    // regex to extract path and line/col from file_info field
    let re = Regex::new(r"^(?P<path>[^\s]+) (?P<line>[0-9]+) (?P<col>[0-9]+)$").ok()?;
    for message in hal_report.messages {
        // preprocess file_info
        let mut file_info = message.file_info[2..].trim_end_matches('}').to_string();
        file_info.retain(|x| x != '"' && x != '\\');
        if let Some(caps) = re.captures(&file_info) {
            if let Ok(file_path) = Url::from_file_path(absolute_path(&caps["path"]).ok()?) {
                if let Ok(line) = &caps["line"].parse::<u64>() {
                    if let Ok(col) = &caps["col"].parse::<u64>() {
                        if uri == &file_path {
                            let pos = Position::new(*line - 1, *col);
                            diags.push(Diagnostic {
                                range: Range::new(pos, pos),
                                severity: Some(message.severity.into()),
                                code: Some(NumberOrString::String(message.id)),
                                source: Some("HAL".to_string()),
                                message: message.info,
                                related_information: None,
                                tags: None,
                            })
                        }
                    }
                }
            }
        }
    }
    Some(diags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostics() {
        let uri =
            Url::from_file_path(absolute_path("test_data/diag/diag_test.sv").unwrap()).unwrap();
        let expected = PublishDiagnosticsParams::new(
            uri.clone(),
            vec![Diagnostic::new(
                Range::new(Position::new(3, 13), Position::new(3, 13)),
                Some(DiagnosticSeverity::Error),
                None,
                Some("slang".to_owned()),
                " cannot refer to element 2 of \'logic[1:0]\'".to_owned(),
                None,
                None,
            )],
            None,
        );
        assert_eq!(get_diagnostics(uri.clone(), vec![uri], false), expected);
    }

    #[test]
    fn test_unsaved_file() {
        let uri = Url::parse("file://test.sv").unwrap();
        get_diagnostics(uri.clone(), vec![uri], false);
    }

    // There's not really a good way to test the HAL linter
    // #[test]
    // fn test_hal_lint() {
    // let uri =
    // Url::from_file_path(absolute_path("test_data/diag/diag_test.sv").unwrap()).unwrap();
    // let paths = vec![
    // absolute_path("test_data/diag/diag_test.sv").unwrap(),
    // absolute_path("test_data/simple_bus.svh").unwrap(),
    // ];
    // dbg!(hal_lint(&uri, paths));
    // panic!();
    // }
}
