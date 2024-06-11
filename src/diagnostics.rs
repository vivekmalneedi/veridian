use crate::server::ProjectConfig;
#[cfg(feature = "slang")]
use path_clean::PathClean;
use regex::Regex;
use ropey::Rope;
#[cfg(feature = "slang")]
use std::env::current_dir;
#[cfg(feature = "slang")]
use std::path::Path;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tower_lsp::lsp_types::*;
#[cfg(feature = "slang")]
use veridian_slang::slang_compile;
use walkdir::DirEntry;
#[cfg(feature = "slang")]
use walkdir::WalkDir;

#[cfg(feature = "slang")]
pub fn get_diagnostics(
    uri: Url,
    rope: &Rope,
    files: Vec<Url>,
    conf: &ProjectConfig,
) -> PublishDiagnosticsParams {
    if !(cfg!(test) && (uri.to_string().starts_with("file:///test"))) {
        let paths = get_paths(files, conf.auto_search_workdir);
        let mut diagnostics = {
            if conf.verilator.syntax.enabled {
                if let Ok(path) = uri.to_file_path() {
                    match verilator_syntax(
                        rope,
                        path,
                        &conf.verilator.syntax.path,
                        &conf.verilator.syntax.args,
                    ) {
                        Some(diags) => diags,
                        None => Vec::new(),
                    }
                } else {
                    Vec::new()
                }
            } else if conf.verible.syntax.enabled {
                match verible_syntax(rope, &conf.verible.syntax.path, &conf.verible.syntax.args) {
                    Some(diags) => diags,
                    None => Vec::new(),
                }
            } else {
                Vec::new()
            }
        };
        diagnostics.append(&mut parse_report(
            uri.clone(),
            slang_compile(paths).unwrap(),
        ));
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
pub fn get_diagnostics(
    uri: Url,
    rope: &Rope,
    #[allow(unused_variables)] files: Vec<Url>,
    conf: &ProjectConfig,
) -> PublishDiagnosticsParams {
    if !(cfg!(test) && (uri.to_string().starts_with("file:///test"))) {
        let diagnostics = {
            if conf.verilator.syntax.enabled {
                if let Ok(path) = uri.to_file_path() {
                    match verilator_syntax(
                        rope,
                        path,
                        &conf.verilator.syntax.path,
                        &conf.verilator.syntax.args,
                    ) {
                        Some(diags) => diags,
                        None => Vec::new(),
                    }
                } else {
                    Vec::new()
                }
            } else if conf.verible.syntax.enabled {
                match verible_syntax(rope, &conf.verible.syntax.path, &conf.verible.syntax.args) {
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

/// recursively find source file paths from working directory
/// and open files
#[cfg(feature = "slang")]
fn get_paths(files: Vec<Url>, search_workdir: bool) -> Vec<PathBuf> {
    // check recursively from working dir for source files
    let mut paths: Vec<PathBuf> = Vec::new();
    if search_workdir {
        let walker = WalkDir::new(".").into_iter();
        for entry in walker.filter_entry(|e| !is_hidden(e)) {
            let entry = entry.unwrap();
            if entry.file_type().is_file() {
                let extension = entry.path().extension().unwrap();

                if extension == "sv" || extension == "svh" || extension == "v" || extension == "vh"
                {
                    paths.push(entry.path().to_path_buf());
                }
            }
        }
    }

    // check recursively from opened files for source files
    for file in files {
        if let Ok(path) = file.to_file_path() {
            if !paths.contains(&path) {
                let walker = WalkDir::new(path.parent().unwrap()).into_iter();
                for entry in walker.filter_entry(|e| !is_hidden(e)).flatten() {
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
/// parse a report from slang
fn parse_report(uri: Url, report: String) -> Vec<Diagnostic> {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    for line in report.lines() {
        let diag: Vec<&str> = line.splitn(5, ':').collect();
        if absolute_path(diag.first().unwrap()) == uri.to_file_path().unwrap().as_os_str() {
            let pos = Position::new(
                diag.get(1).unwrap().parse::<u32>().unwrap() - 1,
                diag.get(2).unwrap().parse::<u32>().unwrap() - 1,
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
        " error" => Some(DiagnosticSeverity::ERROR),
        " warning" => Some(DiagnosticSeverity::WARNING),
        " note" => Some(DiagnosticSeverity::INFORMATION),
        _ => None,
    }
}

#[cfg(feature = "slang")]
// convert relative path to absolute
fn absolute_path(path_str: &str) -> PathBuf {
    let path = Path::new(path_str);
    current_dir().unwrap().join(path).clean()
}

/// convert captured severity string to DiagnosticSeverity
fn verilator_severity(severity: &str) -> Option<DiagnosticSeverity> {
    match severity {
        "Error" => Some(DiagnosticSeverity::ERROR),
        s if s.starts_with("Warning") => Some(DiagnosticSeverity::WARNING),
        // NOTE: afaik, verilator doesn't have an info or hint severity
        _ => Some(DiagnosticSeverity::INFORMATION),
    }
}

/// syntax checking using verilator --lint-only
fn verilator_syntax(
    rope: &Rope,
    file_path: PathBuf,
    verilator_syntax_path: &str,
    verilator_syntax_args: &[String],
) -> Option<Vec<Diagnostic>> {
    let mut child = Command::new(verilator_syntax_path)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .args(verilator_syntax_args)
        .arg(file_path.to_str()?)
        .spawn()
        .ok()?;

    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"%(?P<severity>Error|Warning)(-(?P<warning_type>[A-Z0-9_]+))?: [^:]+:(?P<line>\d+):((?P<col>\d+):)? ?(?P<message>.*)",
        )
        .unwrap()
    });
    // write file to stdin, read output from stdout
    rope.write_to(child.stdin.as_mut()?).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        let mut diags: Vec<Diagnostic> = Vec::new();
        let raw_output = String::from_utf8(output.stderr).ok()?;
        let filtered_output = raw_output
            .lines()
            .filter(|line| line.starts_with('%'))
            .collect::<Vec<&str>>();
        for error in filtered_output {
            let caps = match re.captures(error) {
                Some(caps) => caps,
                None => break, // return accumulated diagnostics
            };
            let severity = verilator_severity(caps.name("severity")?.as_str());
            let line: u32 = caps.name("line")?.as_str().to_string().parse().ok()?;
            let col: u32 = caps.name("col").map_or("1", |m| m.as_str()).parse().ok()?;
            let pos = Position::new(line - 1, col - 1);
            let msg = match severity {
                Some(DiagnosticSeverity::ERROR) => caps.name("message")?.as_str().to_string(),
                Some(DiagnosticSeverity::WARNING) => format!(
                    "{}: {}",
                    caps.name("warning_type")?.as_str(),
                    caps.name("message")?.as_str()
                ),
                _ => "".to_string(),
            };
            diags.push(Diagnostic::new(
                Range::new(pos, pos),
                severity,
                None,
                Some("verilator".to_string()),
                msg,
                None,
                None,
            ));
        }
        Some(diags)
    } else {
        None
    }
}

/// syntax checking using verible-verilog-syntax
fn verible_syntax(
    rope: &Rope,
    verible_syntax_path: &str,
    verible_syntax_args: &[String],
) -> Option<Vec<Diagnostic>> {
    let mut child = Command::new(verible_syntax_path)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .args(verible_syntax_args)
        .arg("-")
        .spawn()
        .ok()?;

    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"^.+:(?P<line>\d*):(?P<startcol>\d*)(?:-(?P<endcol>\d*))?:\s(?P<message>.*)\s.*$",
        )
        .unwrap()
    });
    // write file to stdin, read output from stdout
    rope.write_to(child.stdin.as_mut()?).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        let mut diags: Vec<Diagnostic> = Vec::new();
        let raw_output = String::from_utf8(output.stdout).ok()?;
        for error in raw_output.lines() {
            let caps = re.captures(error)?;
            let line: u32 = caps.name("line")?.as_str().parse().ok()?;
            let startcol: u32 = caps.name("startcol")?.as_str().parse().ok()?;
            let endcol: Option<u32> = match caps.name("endcol").map(|e| e.as_str().parse()) {
                Some(Ok(e)) => Some(e),
                None => None,
                Some(Err(_)) => return None,
            };
            let start_pos = Position::new(line - 1, startcol - 1);
            let end_pos = Position::new(line - 1, endcol.unwrap_or(startcol) - 1);
            diags.push(Diagnostic::new(
                Range::new(start_pos, end_pos),
                Some(DiagnosticSeverity::ERROR),
                None,
                Some("verible".to_string()),
                caps.name("message")?.as_str().to_string(),
                None,
                None,
            ));
        }
        Some(diags)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::test_init;
    use std::fs::File;
    use std::io::Write;
    use tempdir::TempDir;

    #[test]
    #[cfg(feature = "slang")]
    fn test_diagnostics() {
        test_init();
        let uri = Url::from_file_path(absolute_path("test_data/diag/diag_test.sv")).unwrap();
        let expected = PublishDiagnosticsParams::new(
            uri.clone(),
            vec![Diagnostic::new(
                Range::new(Position::new(3, 13), Position::new(3, 13)),
                Some(DiagnosticSeverity::WARNING),
                None,
                Some("slang".to_owned()),
                " cannot refer to element 2 of \'logic[1:0]\' [-Windex-oob]".to_owned(),
                None,
                None,
            )],
            None,
        );
        let diag = get_diagnostics(
            uri.clone(),
            &Rope::default(),
            vec![uri],
            &ProjectConfig::default(),
        );
        assert_eq!(diag.uri, expected.uri);
        assert_eq!(diag.version, expected.version);
        assert_eq!(diag.diagnostics.last(), expected.diagnostics.last());
    }

    #[test]
    fn test_unsaved_file() {
        test_init();
        let uri = Url::parse("file://test.sv").unwrap();
        get_diagnostics(
            uri.clone(),
            &Rope::default(),
            vec![uri],
            &ProjectConfig::default(),
        );
    }

    #[test]
    fn test_verible_syntax() {
        let text = r#"module test;
    logic abc;
    logic abcd;

  a
endmodule
"#;
        let doc = Rope::from_str(text);
        let errors = verible_syntax(&doc, "verible-verilog-syntax", &[])
            .expect("verible-verilog-syntax not found, test can not run");
        let expected: Vec<Diagnostic> = vec![Diagnostic {
            range: Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 8,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            source: Some("verible".to_string()),
            message: "syntax error at token".to_string(),
            related_information: None,
            tags: None,
            code_description: None,
            data: None,
        }];
        assert_eq!(errors, expected);
    }

    #[test]
    fn test_verilator_syntax() {
        let text = r#"module test;
    logic abc;
    logic abcd;

  a
endmodule
"#;
        let doc = Rope::from_str(text);

        // verilator can't read from stdin so we must create a temp dir to place our
        // test file
        let dir = TempDir::new("verilator_test").unwrap();
        let file_path_1 = dir.path().join("test.sv");
        let mut f = File::create(&file_path_1).unwrap();
        f.write_all(text.as_bytes()).unwrap();
        f.sync_all().unwrap();

        let errors = verilator_syntax(
            &doc,
            file_path_1,
            "verilator",
            &[
                "--lint-only".to_string(),
                "--sv".to_string(),
                "-Wall".to_string(),
            ],
        )
        .expect("verilator not found, test can not run");

        drop(f);
        dir.close().unwrap();

        let expected: Vec<Diagnostic> = vec![Diagnostic {
            range: Range {
                start: Position {
                    line: 5,
                    character: 0,
                },
                end: Position {
                    line: 5,
                    character: 0,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            source: Some("verilator".to_string()),
            message: "syntax error, unexpected endmodule, expecting IDENTIFIER or randomize"
                .to_string(),
            related_information: None,
            tags: None,
            code_description: None,
            data: None,
        }];
        assert_eq!(errors[0].severity, expected[0].severity);
        assert_eq!(errors[0].range.start.line, expected[0].range.start.line);
        assert_eq!(errors[0].range.end.line, expected[0].range.end.line);
        assert!(errors[0].message.contains("syntax error"));
    }
}
