use crate::sources::LSPSupport;
use log::trace;
use ropey::Rope;
use tower_lsp::lsp_types::*;

/// cleanup the text of a definition so it can be included in completions
pub fn clean_type_str(type_str: &str, ident: &str) -> String {
    let endings: &[_] = &[';', ','];
    // remove anything after an equals sign
    let eq_offset = type_str.find('=').unwrap_or_else(|| type_str.len());
    let mut result = type_str.to_string();
    result.replace_range(eq_offset.., "");
    result
        .trim_start()
        .trim_end()
        .trim_end_matches(endings)
        .trim_end_matches(ident)
        .trim_end()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
        .replace("[ ", "[")
        .replace(" ]", "]")
        .replace(" : ", ":")
}

/// A definition of any SystemVerilog variable or construct
pub trait Definition: std::fmt::Debug + Sync + Send {
    // identifier
    fn ident(&self) -> String;
    // byte index in file of definition
    fn byte_idx(&self) -> usize;
    // url pointing to the file the definition is in
    fn url(&self) -> Url;
    // cleaned up text of the definition
    fn type_str(&self) -> String;
    // the kind of this definition, for use in completions
    fn completion_kind(&self) -> CompletionItemKind;
    // the kind of this definition, for use in showing document symbols
    // for some reason this kind is different than CompletionItemKind
    fn symbol_kind(&self) -> SymbolKind;
    // the kind of this definition, simplified for internal use
    fn def_type(&self) -> &DefinitionType;
    // whether the definition identifier starts with the given token
    fn starts_with(&self, token: &str) -> bool;
    // constructs the completion for this definition
    fn completion(&self) -> CompletionItem;
    fn dot_completion(&self, scope_tree: &GenericScope) -> Vec<CompletionItem>;
}

pub trait Scope: std::fmt::Debug + Definition + Sync + Send {
    // the start byte of this scope
    fn start(&self) -> usize;
    // the end byte of this scope
    fn end(&self) -> usize;
    // all the within this scope
    fn defs(&self) -> &Vec<Box<dyn Definition>>;
    // all the scopes within this scope, ex. task inside a module
    fn scopes(&self) -> &Vec<Box<dyn Scope>>;
    // the definition of this scope
    fn definition(&self) -> GenericDec {
        GenericDec {
            ident: self.ident(),
            byte_idx: self.byte_idx(),
            url: self.url(),
            type_str: self.type_str(),
            completion_kind: self.completion_kind(),
            symbol_kind: self.symbol_kind(),
            def_type: DefinitionType::GenericScope,
        }
    }
    /// return a completion from the scope tree, this function should be called on the global scope
    fn get_completion(&self, token: &str, byte_idx: usize, url: &Url) -> Vec<CompletionItem> {
        let mut completions: Vec<CompletionItem> = Vec::new();
        // first we need to go down the scope tree, to the scope the user is invoking a completion
        // in
        for scope in self.scopes() {
            if &scope.url() == url && scope.start() <= byte_idx && byte_idx <= scope.end() {
                completions = scope.get_completion(token, byte_idx, url);
                break;
            }
        }
        // now that we are in the users scope, we can attempt to find a relevant completion
        // we proceed back upwards through the scope tree, adding any definitions that match
        // the users token
        let completion_idents: Vec<String> = completions.iter().map(|x| x.label.clone()).collect();
        for def in self.defs() {
            if !completion_idents.contains(&def.ident()) && def.starts_with(token) {
                completions.push(def.completion());
            }
        }
        for scope in self.scopes() {
            if scope.starts_with(token) {
                completions.push(scope.completion());
            }
        }
        completions
    }

    /// return a dot completion from the scope tree, this function should be called on the global
    /// scope
    fn get_dot_completion(
        &self,
        token: &str,
        byte_idx: usize,
        url: &Url,
        scope_tree: &GenericScope,
    ) -> Vec<CompletionItem> {
        trace!("dot entering: {}, token: {}", self.ident(), token);
        trace!("{:?}", self.scopes());
        // first we need to go down the scope tree, to the scope the user is invoking a completion
        // in
        for scope in self.scopes() {
            trace!(
                "{}, {}, {}, {}",
                scope.ident(),
                byte_idx,
                scope.start(),
                scope.end()
            );
            if &scope.url() == url && scope.start() <= byte_idx && byte_idx <= scope.end() {
                eprintln!("checking dot completion: {}", scope.ident());
                let result = scope.get_dot_completion(token, byte_idx, url, scope_tree);
                if !result.is_empty() {
                    return result;
                }
            }
        }
        // now that we are in the users scope, we can attempt to find the relevant definition
        // we proceed back upwards through the scope tree, and if a definition matches our token,
        // we invoke dot completion on that definition and pass it the syntax tree
        for def in self.defs() {
            trace!("def: {:?}", def);
            if def.starts_with(token) {
                trace!("complete def: {:?}", def);
                return def.dot_completion(scope_tree);
            }
        }
        for scope in self.scopes() {
            if scope.starts_with(token) {
                trace!("found dot-completion scope: {}", scope.ident());
                return scope.dot_completion(scope_tree);
            }
        }
        Vec::new()
    }

    /// return a definition from the scope tree, this function should be called on the global
    /// scope
    fn get_definition(&self, token: &str, byte_idx: usize, url: &Url) -> Option<GenericDec> {
        let mut definition: Option<GenericDec> = None;
        for scope in self.scopes() {
            if &scope.url() == url && scope.start() <= byte_idx && byte_idx <= scope.end() {
                definition = scope.get_definition(token, byte_idx, url);
                break;
            }
        }
        if definition.is_none() {
            for def in self.defs() {
                if def.ident() == token {
                    return Some(GenericDec {
                        ident: def.ident(),
                        byte_idx: def.byte_idx(),
                        url: def.url(),
                        type_str: def.type_str(),
                        completion_kind: def.completion_kind(),
                        symbol_kind: def.symbol_kind(),
                        def_type: DefinitionType::Net,
                    });
                }
            }
            for scope in self.scopes() {
                if scope.ident() == token {
                    return Some(scope.definition());
                }
            }
        }
        definition
    }
    /// returns all symbols in a document
    fn document_symbols(&self, uri: &Url, doc: &Rope) -> Vec<DocumentSymbol> {
        let mut symbols: Vec<DocumentSymbol> = Vec::new();
        for scope in self.scopes() {
            if &scope.url() == uri {
                #[allow(deprecated)]
                symbols.push(DocumentSymbol {
                    name: scope.ident(),
                    detail: Some(scope.type_str()),
                    kind: scope.symbol_kind(),
                    deprecated: None,
                    range: Range::new(doc.byte_to_pos(scope.start()), doc.byte_to_pos(scope.end())),
                    selection_range: Range::new(
                        doc.byte_to_pos(scope.byte_idx()),
                        doc.byte_to_pos(scope.byte_idx() + scope.ident().len()),
                    ),
                    children: Some(scope.document_symbols(uri, doc)),
                })
            }
        }
        for def in self.defs() {
            #[allow(deprecated)]
            symbols.push(DocumentSymbol {
                name: def.ident(),
                detail: Some(def.type_str()),
                kind: def.symbol_kind(),
                deprecated: None,
                range: Range::new(
                    doc.byte_to_pos(def.byte_idx()),
                    doc.byte_to_pos(def.byte_idx() + def.ident().len()),
                ),
                selection_range: Range::new(
                    doc.byte_to_pos(def.byte_idx()),
                    doc.byte_to_pos(def.byte_idx() + def.ident().len()),
                ),
                children: None,
            })
        }
        symbols
    }

    /// highlight all references of a symbol
    fn document_highlights(
        &self,
        uri: &Url,
        doc: &Rope,
        // all references in the doc's syntax tree
        references: Vec<(String, usize)>,
        // byte_idx of symbol definition
        byte_idx: usize,
    ) -> Vec<DocumentHighlight> {
        // to find references we need to grab references from locations downward from the
        // definition
        for scope in self.scopes() {
            if &scope.url() == uri && scope.start() <= byte_idx && byte_idx <= scope.end() {
                return scope.document_highlights(uri, doc, references, byte_idx);
            }
        }
        // we should now be in the scope of the definition, so we can grab all references
        // in this scope. This also grabs references below this scope.
        references
            .iter()
            .filter(|x| self.start() <= x.1 && x.1 <= self.end())
            .map(|x| DocumentHighlight {
                range: Range::new(doc.byte_to_pos(x.1), doc.byte_to_pos(x.1 + x.0.len())),
                kind: None,
            })
            .collect()
    }
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
    Class,
}

#[derive(Debug)]
pub struct PortDec {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub completion_kind: CompletionItemKind,
    pub symbol_kind: SymbolKind,
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
            completion_kind: CompletionItemKind::Property,
            symbol_kind: SymbolKind::Property,
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
    fn completion_kind(&self) -> CompletionItemKind {
        self.completion_kind
    }
    fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
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
            kind: Some(self.completion_kind),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &GenericScope) -> Vec<CompletionItem> {
        for scope in &scope_tree.scopes {
            if let Some(interface) = &self.interface {
                if &scope.ident() == interface {
                    return match &self.modport {
                        Some(modport) => {
                            for def in scope.defs() {
                                if def.starts_with(&modport) {
                                    return def.dot_completion(scope_tree);
                                }
                            }
                            Vec::new()
                        }
                        None => scope
                            .defs()
                            .iter()
                            .filter(|x| !x.starts_with(&scope.ident()))
                            .map(|x| x.completion())
                            .collect(),
                    };
                }
            }
        }
        Vec::new()
    }
}

#[derive(Debug)]
pub struct GenericDec {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub completion_kind: CompletionItemKind,
    pub symbol_kind: SymbolKind,
    def_type: DefinitionType,
}

impl GenericDec {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            url: url.clone(),
            type_str: String::new(),
            completion_kind: CompletionItemKind::Variable,
            symbol_kind: SymbolKind::Unknown,
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
    fn completion_kind(&self) -> CompletionItemKind {
        self.completion_kind
    }
    fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
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
            kind: Some(self.completion_kind),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, _: &GenericScope) -> Vec<CompletionItem> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct PackageImport {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub completion_kind: CompletionItemKind,
    pub symbol_kind: SymbolKind,
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
            completion_kind: CompletionItemKind::Text,
            symbol_kind: SymbolKind::Namespace,
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
    fn completion_kind(&self) -> CompletionItemKind {
        self.completion_kind
    }
    fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
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
            kind: Some(self.completion_kind),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, _: &GenericScope) -> Vec<CompletionItem> {
        Vec::new()
    }
}

#[derive(Debug)]
pub struct SubDec {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub completion_kind: CompletionItemKind,
    pub symbol_kind: SymbolKind,
    def_type: DefinitionType,
    pub start: usize,
    pub end: usize,
    pub defs: Vec<Box<dyn Definition>>,
    pub scopes: Vec<Box<dyn Scope>>,
}

impl SubDec {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            url: url.clone(),
            type_str: String::new(),
            completion_kind: CompletionItemKind::Function,
            symbol_kind: SymbolKind::Function,
            def_type: DefinitionType::Subroutine,
            start: 0,
            end: 0,
            defs: Vec::new(),
            scopes: Vec::new(),
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
    fn completion_kind(&self) -> CompletionItemKind {
        self.completion_kind
    }
    fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
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
            kind: Some(self.completion_kind),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, _: &GenericScope) -> Vec<CompletionItem> {
        Vec::new()
    }
}

impl Scope for SubDec {
    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }
    fn defs(&self) -> &Vec<Box<dyn Definition>> {
        &self.defs
    }

    fn scopes(&self) -> &Vec<Box<dyn Scope>> {
        &self.scopes
    }
}

#[derive(Debug)]
pub struct ModportDec {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub completion_kind: CompletionItemKind,
    pub symbol_kind: SymbolKind,
    def_type: DefinitionType,
    pub ports: Vec<Box<dyn Definition>>,
}

impl ModportDec {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            url: url.clone(),
            type_str: String::new(),
            completion_kind: CompletionItemKind::Interface,
            symbol_kind: SymbolKind::Interface,
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
    fn completion_kind(&self) -> CompletionItemKind {
        self.completion_kind
    }
    fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
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
            kind: Some(self.completion_kind),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, _: &GenericScope) -> Vec<CompletionItem> {
        self.ports.iter().map(|x| x.completion()).collect()
    }
}

#[derive(Debug)]
pub struct ModInst {
    pub ident: String,
    pub byte_idx: usize,
    pub url: Url,
    pub type_str: String,
    pub completion_kind: CompletionItemKind,
    pub symbol_kind: SymbolKind,
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
            completion_kind: CompletionItemKind::Module,
            symbol_kind: SymbolKind::Module,
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
    fn completion_kind(&self) -> CompletionItemKind {
        self.completion_kind
    }
    fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
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
            kind: Some(self.completion_kind),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &GenericScope) -> Vec<CompletionItem> {
        for scope in &scope_tree.scopes {
            if scope.ident() == self.mod_ident {
                return scope
                    .defs()
                    .iter()
                    .filter(|x| !x.starts_with(&scope.ident()))
                    .map(|x| x.completion())
                    .collect();
            }
        }
        Vec::new()
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
    pub completion_kind: CompletionItemKind,
    pub symbol_kind: SymbolKind,
    def_type: DefinitionType,
    pub defs: Vec<Box<dyn Definition>>,
    pub scopes: Vec<Box<dyn Scope>>,
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
            completion_kind: CompletionItemKind::Module,
            symbol_kind: SymbolKind::Module,
            def_type: DefinitionType::GenericScope,
            defs: Vec::new(),
            scopes: Vec::new(),
        }
    }
    #[cfg(test)]
    pub fn contains_scope(&self, scope_ident: &str) -> bool {
        for scope in &self.scopes {
            if scope.starts_with(scope_ident) {
                return true;
            }
        }
        false
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
    fn completion_kind(&self) -> CompletionItemKind {
        self.completion_kind
    }
    fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
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
            kind: Some(self.completion_kind),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &GenericScope) -> Vec<CompletionItem> {
        for scope in scope_tree.scopes() {
            if scope.ident() == self.ident {
                return scope
                    .defs()
                    .iter()
                    .filter(|x| !x.starts_with(&scope.ident()))
                    .map(|x| x.completion())
                    .collect();
            }
        }
        Vec::new()
    }
}

impl Scope for GenericScope {
    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }

    fn defs(&self) -> &Vec<Box<dyn Definition>> {
        &self.defs
    }

    fn scopes(&self) -> &Vec<Box<dyn Scope>> {
        &self.scopes
    }
}

#[derive(Debug)]
pub struct ClassDec {
    pub ident: String,
    pub byte_idx: usize,
    pub start: usize,
    pub end: usize,
    pub url: Url,
    pub type_str: String,
    pub completion_kind: CompletionItemKind,
    pub symbol_kind: SymbolKind,
    def_type: DefinitionType,
    pub defs: Vec<Box<dyn Definition>>,
    pub scopes: Vec<Box<dyn Scope>>,
    // class, package
    pub extends: (Vec<String>, Option<String>),
    // class, package
    pub implements: Vec<(String, Option<String>)>,
    pub interface: bool,
}

impl ClassDec {
    pub fn new(url: &Url) -> Self {
        Self {
            ident: String::new(),
            byte_idx: 0,
            start: 0,
            end: 0,
            url: url.clone(),
            type_str: String::new(),
            completion_kind: CompletionItemKind::Class,
            symbol_kind: SymbolKind::Class,
            def_type: DefinitionType::Class,
            defs: Vec::new(),
            scopes: Vec::new(),
            extends: (Vec::new(), None),
            implements: Vec::new(),
            interface: false,
        }
    }
}

impl Definition for ClassDec {
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
    fn completion_kind(&self) -> CompletionItemKind {
        self.completion_kind
    }
    fn symbol_kind(&self) -> SymbolKind {
        self.symbol_kind
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
            kind: Some(self.completion_kind),
            ..CompletionItem::default()
        }
    }
    fn dot_completion(&self, scope_tree: &GenericScope) -> Vec<CompletionItem> {
        for scope in scope_tree.scopes() {
            if scope.ident() == self.ident {
                return scope
                    .defs()
                    .iter()
                    .filter(|x| !x.starts_with(&scope.ident()))
                    .map(|x| x.completion())
                    .collect();
            }
        }
        Vec::new()
    }
}

impl Scope for ClassDec {
    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }

    fn defs(&self) -> &Vec<Box<dyn Definition>> {
        &self.defs
    }

    fn scopes(&self) -> &Vec<Box<dyn Scope>> {
        &self.scopes
    }
}
