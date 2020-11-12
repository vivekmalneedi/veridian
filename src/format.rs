use crate::server::LSPServer;
use crate::sources::LSPSupport;
use ropey::Rope;
use std::process::{Command, Stdio};
use tower_lsp::lsp_types::*;

impl LSPServer {
    pub fn formatting(&self, params: DocumentFormattingParams) -> Option<Vec<TextEdit>> {
        let uri = params.text_document.uri;
        let file_id = self.srcs.get_id(&uri).to_owned();
        self.srcs.wait_parse_ready(file_id, false);
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;

        let conf = self.conf.read().unwrap();
        if conf.format {
            Some(vec![TextEdit::new(
                Range::new(
                    file.text.char_to_pos(0),
                    file.text.char_to_pos(file.text.len_chars() - 1),
                ),
                format_document(&file.text, None, &conf.verible_format_path)?,
            )])
        } else {
            None
        }
    }

    pub fn range_formatting(&self, params: DocumentRangeFormattingParams) -> Option<Vec<TextEdit>> {
        let uri = params.text_document.uri;
        let file_id = self.srcs.get_id(&uri).to_owned();
        self.srcs.wait_parse_ready(file_id, false);
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;

        let conf = self.conf.read().unwrap();
        if conf.format {
            Some(vec![TextEdit::new(
                file.text.char_range_to_range(0..file.text.len_chars()),
                format_document(&file.text, Some(params.range), &conf.verible_format_path)?,
            )])
        } else {
            None
        }
    }
}

pub fn format_document(
    rope: &Rope,
    range: Option<Range>,
    verible_format_path: &str,
) -> Option<String> {
    let mut child = Command::new(verible_format_path);
    child
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(r) = range {
        child
            .arg("--lines")
            .arg(format!("{}-{}", r.start.line + 1, r.end.line + 1));
    }
    let mut child = child.arg("-").spawn().ok()?;

    rope.write_to(child.stdin.as_mut()?).ok()?;
    let output = child.wait_with_output().ok()?;
    if output.status.success() {
        let raw_output = String::from_utf8(output.stdout).ok()?;
        Some(raw_output)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::ProjectConfig;
    use which::which;

    #[test]
    fn test_formatting() {
        let text = r#"
module test;
  logic a;
   logic b;
endmodule"#;
        let text_fixed = r#"
module test;
  logic a;
  logic b;
endmodule
"#;
        let doc = Rope::from_str(&text);
        if which("verible-verilog-format").is_ok() {
            assert_eq!(
                format_document(&doc, None, &ProjectConfig::default().verible_format_path).unwrap(),
                text_fixed.to_string()
            );
        }
    }

    #[test]
    fn test_range_formatting() {
        let text = r#"module t1;
    logic a;
 logic b;
         logic c;
endmodule


module t2;
    logic a;
 logic b;
         logic c;
endmodule"#;

        let text_fixed = r#"module t1;
  logic a;
  logic b;
  logic c;
endmodule


module t2;
    logic a;
 logic b;
         logic c;
endmodule
"#;
        let doc = Rope::from_str(&text);
        if which("verible-verilog-format").is_ok() {
            assert_eq!(
                format_document(
                    &doc,
                    Some(Range::new(Position::new(0, 0), Position::new(4, 9))),
                    &ProjectConfig::default().verible_format_path
                )
                .unwrap(),
                text_fixed.to_string()
            );
        }
    }
}
