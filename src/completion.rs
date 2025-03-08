use crate::server::LSPServer;
use crate::sources::LSPSupport;
use log::{debug, trace};
use ropey::{Rope, RopeSlice};
use tower_lsp::lsp_types::*;

pub mod keyword;
use keyword::*;

impl LSPServer {
    pub fn completion(&self, params: CompletionParams) -> Option<CompletionResponse> {
        debug!("completion requested");
        trace!("{:#?}", &params);
        let doc = params.text_document_position;
        let files = self.srcs.files.lock().unwrap();
        let file = files.get(&doc.text_document.uri)?;
        let token = get_completion_token(
            &file.text,
            file.text.line(doc.position.line as usize),
            doc.position,
        );
        let response = match params.context {
            Some(context) => match context.trigger_kind {
                CompletionTriggerKind::TRIGGER_CHARACTER => {
                    debug!(
                        "trigger char completion: {}",
                        context.trigger_character.clone()?.as_str()
                    );
                    match context.trigger_character?.as_str() {
                        "." => Some(CompletionList {
                            is_incomplete: false,
                            items: self
                                .srcs
                                .get_dot_completions(
                                    token.trim_end_matches('.'),
                                    file.text.pos_to_byte(&doc.position),
                                    &doc.text_document.uri,
                                )
                                .iter()
                                .map(|s| s.to_completion(&file.text))
                                .collect(),
                        }),
                        "$" => Some(CompletionList {
                            is_incomplete: false,
                            items: other_completions(SYS_TASKS),
                        }),
                        "`" => Some(CompletionList {
                            is_incomplete: false,
                            items: other_completions(DIRECTIVES),
                        }),
                        _ => None,
                    }
                }
                CompletionTriggerKind::TRIGGER_FOR_INCOMPLETE_COMPLETIONS => None,
                CompletionTriggerKind::INVOKED => {
                    debug!("Invoked Completion");
                    let mut comps: Vec<CompletionItem> = self
                        .srcs
                        .get_completions(
                            &token,
                            file.text.pos_to_byte(&doc.position),
                            &doc.text_document.uri,
                        )
                        .iter()
                        .map(|s| s.to_completion(&file.text))
                        .collect();
                    // complete keywords
                    comps.extend::<Vec<CompletionItem>>(
                        keyword_completions(KEYWORDS)
                            .iter()
                            .filter(|x| x.label.starts_with(&token))
                            .cloned()
                            .collect(),
                    );
                    Some(CompletionList {
                        is_incomplete: false,
                        items: comps,
                    })
                }
                _ => None,
            },
            None => {
                let trigger = prev_char(&file.text, &doc.position);
                match trigger {
                    '.' => Some(CompletionList {
                        is_incomplete: false,
                        items: self
                            .srcs
                            .get_dot_completions(
                                token.trim_end_matches('.'),
                                file.text.pos_to_byte(&doc.position),
                                &doc.text_document.uri,
                            )
                            .iter()
                            .map(|s| s.to_completion(&file.text))
                            .collect(),
                    }),
                    '$' => Some(CompletionList {
                        is_incomplete: false,
                        items: other_completions(SYS_TASKS),
                    }),
                    '`' => Some(CompletionList {
                        is_incomplete: false,
                        items: other_completions(DIRECTIVES),
                    }),
                    _ => {
                        let mut comps: Vec<CompletionItem> = self
                            .srcs
                            .get_completions(
                                &token,
                                file.text.pos_to_byte(&doc.position),
                                &doc.text_document.uri,
                            )
                            .iter()
                            .map(|s| s.to_completion(&file.text))
                            .collect();
                        comps.extend::<Vec<CompletionItem>>(
                            keyword_completions(KEYWORDS)
                                .iter()
                                .filter(|x| x.label.starts_with(&token))
                                .cloned()
                                .collect(),
                        );
                        Some(CompletionList {
                            is_incomplete: false,
                            items: comps,
                        })
                    }
                }
            }
        };
        // eprintln!("comp response: {}", now.elapsed().as_millis());
        Some(CompletionResponse::List(response?))
    }
}

/// get the previous non-whitespace character
fn prev_char(text: &Rope, pos: &Position) -> char {
    let char_idx = text.pos_to_char(pos);
    if char_idx > 0 {
        for i in (0..char_idx).rev() {
            let res = text.char(i);
            if !res.is_whitespace() {
                return res;
            }
        }
        ' '
    } else {
        ' '
    }
}

/// attempt to get the token the user was trying to complete, by
/// filtering out characters unneeded for name resolution
fn get_completion_token(text: &Rope, line: RopeSlice, pos: Position) -> String {
    let mut token = String::new();
    let mut line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.prev();
    //TODO: make this a regex
    while c.is_some()
        && (c.unwrap().is_alphanumeric()
            || c.unwrap() == '_'
            || c.unwrap() == '.'
            || c.unwrap() == '['
            || c.unwrap() == ']')
    {
        token.push(c.unwrap());
        c = line_iter.prev();
    }
    let mut result: String = token.chars().rev().collect();
    if result.contains('[') {
        let l_bracket_offset = result.find('[').unwrap_or(result.len());
        result.replace_range(l_bracket_offset.., "");
    }
    if &result == "." {
        // probably a instantiation, the token should be what we're instatiating
        let mut char_iter = text.chars();
        let mut token = String::new();
        for _ in 0..text.pos_to_char(&pos) {
            char_iter.next();
        }
        let mut c = char_iter.prev();

        // go to the last semicolon
        while c.is_some() && (c.unwrap() != ';') {
            c = char_iter.prev();
        }
        // go the the start of the next symbol
        while c.is_some() && !(c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
            c = char_iter.next();
        }
        // then extract the next symbol
        while c.is_some() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
            token.push(c.unwrap());
            c = char_iter.next();
        }
        token
    } else {
        result
    }
}
