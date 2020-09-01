use path_clean::PathClean;
use std::env::current_dir;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tower_lsp::lsp_types::*;
use veridian_slang::slang_compile;
use walkdir::{DirEntry, WalkDir};

pub fn get_diagnostics(uri: Url, files: Vec<Url>) -> PublishDiagnosticsParams {
    let paths = get_paths(files);
    // eprintln!("{:#?}", paths);
    let diagnostics = slang_compile(paths).unwrap();
    // eprintln!("{}", diagnostics);
    PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics: parse_report(uri, diagnostics),
        version: None,
    }
}

fn get_paths(files: Vec<Url>) -> Vec<PathBuf> {
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

    for file in files {
        let path = file.to_file_path().unwrap();
        if !paths.contains(&path) {
            let walker = WalkDir::new(path.parent().unwrap()).into_iter();
            for entry in walker.filter_entry(|e| !is_hidden(e)) {
                let entry = entry.unwrap();
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
    paths
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

fn parse_report(uri: Url, report: String) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for line in report.lines() {
        let diag: Vec<&str> = line.splitn(5, ":").collect();
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

fn slang_severity(severity: &str) -> Option<DiagnosticSeverity> {
    match severity {
        " error" => Some(DiagnosticSeverity::Error),
        " warning" => Some(DiagnosticSeverity::Warning),
        " note" => Some(DiagnosticSeverity::Information),
        _ => None,
    }
}

fn absolute_path(path_str: &str) -> io::Result<PathBuf> {
    let path = Path::new(path_str);
    Ok(current_dir().unwrap().join(path).clean())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostics() {
        let uri =
            Url::from_file_path(absolute_path("tests_rtl/diag/diag_test.sv").unwrap()).unwrap();
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
        assert_eq!(get_diagnostics(uri.clone(), vec![uri.clone()]), expected);
    }
}
