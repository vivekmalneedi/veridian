use crate::sources::Scope;
use std::sync::Arc;
use tower_lsp::lsp_types::*;

fn clean_type_str(type_str: &str, ident: &str) -> String {
    let endings: &[_] = &[';', ','];
    let eq_offset = type_str.find('=').unwrap_or(type_str.len());
    let mut result = type_str.to_string();
    result.replace_range(eq_offset.., "");
    result
        .trim_start()
        .trim_end()
        .trim_end_matches(endings)
        .trim_end_matches(ident)
        .trim_end()
        .to_string()
}

pub trait Definition: std::fmt::Debug + Sync + Send {
    fn ident(&self) -> String;
    fn byte_idx(&self) -> usize;
    fn url(&self) -> Url;
    fn type_str(&self) -> String;
    fn kind(&self) -> CompletionItemKind;
    fn def_type(&self) -> &DefinitionType;
    fn starts_with(&self, token: &str) -> bool;
    fn completion(&self) -> CompletionItem;
    fn dot_completion(&self, scope_tree: &Scope) -> Option<Vec<CompletionItem>>;
}

#[derive(Debug)]
pub enum DefinitionType {
    Port,
    Net,
    Data,
    Modport,
    Subroutine,
    ModuleInstantiation,
    GenericScope,
}

#[derive(Debug)]
pub struct PortDec {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub kind: CompletionItemKind,
    def_type: DefinitionType,
    pub interface: Option<String>,
    pub modport: Option<String>,
}

impl PortDec {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            type_str: String::new(),
            kind: CompletionItemKind::Property,
            def_type: DefinitionType::Port,
            interface: None,
            modport: None,
            url: url.clone(),
        }
    }
}

impl Definition for PortDec {
    fn ident(&self) -> String {
        self.ident.clone()
    }
    fn byte_idx(&self) -> usize {
        self.byte_idx
    }
    fn url(&self) -> Url {
        self.url.clone()
    }
    fn type_str(&self) -> String {
        self.type_str.clone()
    }
    fn kind(&self) -> CompletionItemKind {
        self.kind.clone()
    }
    fn def_type(&self) -> &DefinitionType {
        &self.def_type
    }
    fn starts_with(&self, token: &str) -> bool {
        self.ident.starts_with(token)
    }
    fn completion(&self) -> CompletionItem {
        CompletionItem {
            label: self.ident.clone(),
            detail: Some(clean_type_str(&self.type_str, &self.ident)),
            kind: Some(self.kind.clone()),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &Scope) -> Option<Vec<CompletionItem>> {
        for scope in &scope_tree.scopes {
            if &scope.name == self.interface.as_ref()? {
                return match &self.modport {
                    Some(modport) => {
                        for def in &scope.defs {
                            if def.starts_with(&modport) {
                                return def.dot_completion(scope_tree);
                            }
                        }
                        None
                    }
                    None => Some(
                        scope
                            .defs
                            .iter()
                            .filter(|x| !x.starts_with(&scope.name))
                            .map(|x| x.completion())
                            .collect(),
                    ),
                };
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct GenericDec {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub kind: CompletionItemKind,
    def_type: DefinitionType,
}

impl GenericDec {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            url: url.clone(),
            type_str: String::new(),
            kind: CompletionItemKind::Variable,
            def_type: DefinitionType::Net,
        }
    }
}

impl Definition for GenericDec {
    fn ident(&self) -> String {
        self.ident.clone()
    }
    fn byte_idx(&self) -> usize {
        self.byte_idx
    }
    fn url(&self) -> Url {
        self.url.clone()
    }
    fn type_str(&self) -> String {
        self.type_str.clone()
    }
    fn kind(&self) -> CompletionItemKind {
        self.kind.clone()
    }
    fn def_type(&self) -> &DefinitionType {
        &self.def_type
    }
    fn starts_with(&self, token: &str) -> bool {
        self.ident.starts_with(token)
    }
    fn completion(&self) -> CompletionItem {
        CompletionItem {
            label: self.ident.clone(),
            detail: Some(clean_type_str(&self.type_str, &self.ident)),
            kind: Some(self.kind.clone()),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &Scope) -> Option<Vec<CompletionItem>> {
        None
    }
}

#[derive(Debug)]
pub struct PackageImport {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub kind: CompletionItemKind,
    def_type: DefinitionType,
    pub asterisk: bool,
    pub import_ident: Option<String>,
}

impl PackageImport {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            url: url.clone(),
            type_str: String::new(),
            kind: CompletionItemKind::Variable,
            def_type: DefinitionType::Data,
            asterisk: false,
            import_ident: None,
        }
    }
}

impl Definition for PackageImport {
    fn ident(&self) -> String {
        self.ident.clone()
    }
    fn byte_idx(&self) -> usize {
        self.byte_idx
    }
    fn url(&self) -> Url {
        self.url.clone()
    }
    fn type_str(&self) -> String {
        self.type_str.clone()
    }
    fn kind(&self) -> CompletionItemKind {
        self.kind.clone()
    }
    fn def_type(&self) -> &DefinitionType {
        &self.def_type
    }
    fn starts_with(&self, token: &str) -> bool {
        self.ident.starts_with(token)
    }
    fn completion(&self) -> CompletionItem {
        CompletionItem {
            label: self.ident.clone(),
            detail: Some(clean_type_str(&self.type_str, &self.ident.clone())),
            kind: Some(self.kind.clone()),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &Scope) -> Option<Vec<CompletionItem>> {
        None
    }
}

#[derive(Debug)]
pub struct SubDec {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub kind: CompletionItemKind,
    def_type: DefinitionType,
}

impl SubDec {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            url: url.clone(),
            type_str: String::new(),
            kind: CompletionItemKind::Function,
            def_type: DefinitionType::Subroutine,
        }
    }
}

impl Definition for SubDec {
    fn ident(&self) -> String {
        self.ident.clone()
    }
    fn byte_idx(&self) -> usize {
        self.byte_idx
    }
    fn url(&self) -> Url {
        self.url.clone()
    }
    fn type_str(&self) -> String {
        self.type_str.clone()
    }
    fn kind(&self) -> CompletionItemKind {
        self.kind.clone()
    }
    fn def_type(&self) -> &DefinitionType {
        &self.def_type
    }
    fn starts_with(&self, token: &str) -> bool {
        self.ident.starts_with(token)
    }
    fn completion(&self) -> CompletionItem {
        CompletionItem {
            label: self.ident.clone(),
            detail: Some(clean_type_str(&self.type_str, &self.ident)),
            kind: Some(self.kind.clone()),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &Scope) -> Option<Vec<CompletionItem>> {
        None
    }
}

#[derive(Debug)]
pub struct ModportDec {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub kind: CompletionItemKind,
    def_type: DefinitionType,
    pub ports: Vec<Arc<dyn Definition>>,
}

impl ModportDec {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            url: url.clone(),
            type_str: String::new(),
            kind: CompletionItemKind::Interface,
            def_type: DefinitionType::Modport,
            ports: Vec::new(),
        }
    }
}

impl Definition for ModportDec {
    fn ident(&self) -> String {
        self.ident.clone()
    }
    fn byte_idx(&self) -> usize {
        self.byte_idx
    }
    fn url(&self) -> Url {
        self.url.clone()
    }
    fn type_str(&self) -> String {
        self.type_str.clone()
    }
    fn kind(&self) -> CompletionItemKind {
        self.kind.clone()
    }
    fn def_type(&self) -> &DefinitionType {
        &self.def_type
    }
    fn starts_with(&self, token: &str) -> bool {
        self.ident.starts_with(token)
    }
    fn completion(&self) -> CompletionItem {
        CompletionItem {
            label: self.ident.clone(),
            detail: Some(clean_type_str(&self.type_str, &self.ident)),
            kind: Some(self.kind.clone()),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &Scope) -> Option<Vec<CompletionItem>> {
        Some(self.ports.iter().map(|x| x.completion()).collect())
    }
}

#[derive(Debug)]
pub struct ModInst {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub kind: CompletionItemKind,
    def_type: DefinitionType,
    pub mod_ident: String,
}

impl ModInst {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            url: url.clone(),
            type_str: String::new(),
            kind: CompletionItemKind::Variable,
            def_type: DefinitionType::ModuleInstantiation,
            mod_ident: String::new(),
        }
    }
}

impl Definition for ModInst {
    fn ident(&self) -> String {
        self.ident.clone()
    }
    fn byte_idx(&self) -> usize {
        self.byte_idx
    }
    fn url(&self) -> Url {
        self.url.clone()
    }
    fn type_str(&self) -> String {
        self.type_str.clone()
    }
    fn kind(&self) -> CompletionItemKind {
        self.kind.clone()
    }
    fn def_type(&self) -> &DefinitionType {
        &self.def_type
    }
    fn starts_with(&self, token: &str) -> bool {
        self.ident.starts_with(token)
    }
    fn completion(&self) -> CompletionItem {
        CompletionItem {
            label: self.ident.clone(),
            detail: Some(clean_type_str(&self.type_str, &self.ident)),
            kind: Some(self.kind.clone()),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &Scope) -> Option<Vec<CompletionItem>> {
        for scope in &scope_tree.scopes {
            if &scope.name == &self.mod_ident {
                return Some(
                    scope
                        .defs
                        .iter()
                        .filter(|x| !x.starts_with(&scope.name))
                        .map(|x| x.completion())
                        .collect(),
                );
            }
        }
        None
    }
}

#[derive(Debug)]
pub struct GenericScope {
    pub ident: String,
    pub byte_idx: usize,
    pub start: usize,
    pub end: usize,
    pub url: Url,
    pub type_str: String,
    pub kind: CompletionItemKind,
    def_type: DefinitionType,
    pub defs: Vec<Box<dyn Definition>>,
}

impl GenericScope {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            start: 0,
            end: 0,
            url: url.clone(),
            type_str: String::new(),
            kind: CompletionItemKind::Module,
            def_type: DefinitionType::GenericScope,
            defs: Vec::new(),
        }
    }
}

impl Definition for GenericScope {
    fn ident(&self) -> String {
        self.ident.clone()
    }
    fn byte_idx(&self) -> usize {
        self.byte_idx
    }
    fn url(&self) -> Url {
        self.url.clone()
    }
    fn type_str(&self) -> String {
        self.type_str.clone()
    }
    fn kind(&self) -> CompletionItemKind {
        self.kind.clone()
    }
    fn def_type(&self) -> &DefinitionType {
        &self.def_type
    }
    fn starts_with(&self, token: &str) -> bool {
        self.ident.starts_with(token)
    }
    fn completion(&self) -> CompletionItem {
        CompletionItem {
            label: self.ident.clone(),
            detail: Some(clean_type_str(&self.type_str, &self.ident)),
            kind: Some(self.kind.clone()),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &Scope) -> Option<Vec<CompletionItem>> {
        for scope in &scope_tree.scopes {
            if &scope.name == &self.ident {
                return Some(
                    scope
                        .defs
                        .iter()
                        .filter(|x| !x.starts_with(&scope.name))
                        .map(|x| x.completion())
                        .collect(),
                );
            }
        }
        None
    }
}
