use path_clean::PathClean;
use std::env::current_dir;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use tower_lsp::lsp_types::*;
use verilogls_slang_wrapper::slang_compile;

pub fn get_diagnostics(uri: Url) -> PublishDiagnosticsParams {
    let path = uri.to_file_path().unwrap();
    if !path.exists() {
        return PublishDiagnosticsParams::new(uri, Vec::new(), None);
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    paths.push(path);
    PublishDiagnosticsParams {
        uri: uri.clone(),
        diagnostics: parse_report(uri, slang_compile(paths).unwrap()),
        version: None,
    }
}

fn parse_report(uri: Url, report: String) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for line in report.lines() {
        let diag: Vec<&str> = line.splitn(5, ":").collect();
        if absolute_path(diag.get(0).unwrap()).unwrap()
            == uri.to_file_path().unwrap().as_os_str()
        {
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
        let uri = Url::from_file_path(absolute_path("tests_rtl/diag_test.sv").unwrap())
            .unwrap();
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
        assert_eq!(get_diagnostics(uri), expected);
    }
}
