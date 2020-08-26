use crate::server::LSPServer;
use crate::sources::LSPSupport;
use ropey::{Rope, RopeSlice};
use std::fmt::Display;
use sv_parser::*;
use tower_lsp::lsp_types::*;

impl LSPServer {
    pub fn goto_definition(&self, params: GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        let def = file
            .scope_tree
            .as_ref()?
            .get_definition(&token, file.text.pos_to_byte(&pos))?;
        let def_pos = file.text.byte_to_pos(def.byte_idx);
        Some(GotoDefinitionResponse::Scalar(Location::new(
            doc,
            Range::new(def_pos, def_pos),
        )))
    }

    pub fn hover(&self, params: HoverParams) -> Option<Hover> {
        let doc = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let file_id = self.srcs.get_id(&doc).to_owned();
        let file = self.srcs.get_file(file_id)?;
        let file = file.read().ok()?;
        let token = get_definition_token(file.text.line(pos.line as usize), pos);
        let def = file
            .scope_tree
            .as_ref()?
            .get_definition(&token, file.text.pos_to_byte(&pos))?;
        let def_line = file.text.byte_to_line(def.byte_idx);
        Some(Hover {
            contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                language: "systemverilog".to_owned(),
                value: get_hover(&file.text, def_line),
            })),
            range: None,
        })
    }
}

fn get_definition_token(line: RopeSlice, pos: Position) -> String {
    let mut token = String::new();
    let mut line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.prev();
    while !c.is_none() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
        token.push(c.unwrap());
        c = line_iter.prev();
    }
    token = token.chars().rev().collect();
    line_iter = line.chars();
    for _ in 0..(line.utf16_cu_to_char(pos.character as usize)) {
        line_iter.next();
    }
    let mut c = line_iter.next();
    while !c.is_none() && (c.unwrap().is_alphanumeric() || c.unwrap() == '_') {
        token.push(c.unwrap());
        c = line_iter.next();
    }
    token
}

/*
pub trait Definition: Sync + Send {
    fn get_ident(&self) -> &str;
    fn get_byte_idx(&self) -> usize;
    fn get_type_str(&self) -> &str;
    fn get_kind(&self) -> &CompletionItemKind;
}
*/

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

#[derive(Debug, Clone)]
pub struct Definition {
    pub ident: String,
    pub byte_idx: usize,
    pub type_str: String,
    pub kind: CompletionItemKind,
    interface: Option<String>,
    modport: Option<String>,
    import_ident: Option<String>,
}

impl std::default::Default for Definition {
    fn default() -> Self {
        Definition {
            ident: String::new(),
            byte_idx: 0,
            type_str: String::new(),
            kind: CompletionItemKind::Variable,
            interface: None,
            modport: None,
            import_ident: None,
        }
    }
}

/*
impl Definition for Port {
    fn get_ident(&self) -> &str {
        &self.ident
    }
    fn get_byte_idx(&self) -> usize {
        self.byte_idx
    }
    fn get_type_str(&self) -> &str {
        &self.type_str
    }
    fn get_kind(&self) -> &CompletionItemKind {
        &self.kind
    }
}
*/

fn get_ident(tree: &SyntaxTree, node: RefNode) -> (String, usize) {
    let loc = unwrap_locate!(node).unwrap();
    let ident_str = tree.get_str(loc).unwrap().to_string();
    let byte_idx = tree.get_origin(loc).unwrap().1;
    (ident_str, byte_idx)
}

macro_rules! advance_until_leave {
    ($tokens:ident, $tree:ident, $event_iter:ident, $node:path) => {{
        let mut result: Option<RefNode> = None;
        while let Some(event) = $event_iter.next() {
            match event {
                NodeEvent::Leave(x) => match x {
                    $node(node) => {
                        result = Some($node(node));
                        break;
                    }
                    RefNode::Locate(node) => {
                        $tokens.push(' ');
                        $tokens.push_str($tree.get_str(node)?);
                    }
                    _ => (),
                },
                NodeEvent::Enter(_) => (),
            }
        }
        result
    }};
}

macro_rules! advance_until_enter {
    ($tokens:ident, $tree:ident, $event_iter:ident, $node:path, $type:ty) => {{
        let mut result: Option<$type> = None;
        while let Some(event) = $event_iter.next() {
            match event {
                NodeEvent::Enter(x) => match x {
                    $node(node) => {
                        result = Some(node);
                        break;
                    }
                    RefNode::Locate(node) => {
                        $tokens.push(' ');
                        $tokens.push_str($tree.get_str(node)?);
                    }
                    _ => (),
                },
                NodeEvent::Leave(_) => (),
            }
        }
        result
    }};
}

macro_rules! skip_until_enter {
    ($tree:ident, $event_iter:ident, $node:path, $type:ty) => {{
        let mut result: Option<$type> = None;
        while let Some(event) = $event_iter.next() {
            match event {
                NodeEvent::Enter(x) => match x {
                    $node(node) => {
                        result = Some(node);
                        break;
                    }
                    _ => (),
                },
                NodeEvent::Leave(_) => (),
            }
        }
        result
    }};
}

fn port_dec_ansi(
    tree: &SyntaxTree,
    node: &AnsiPortDeclaration,
    event_iter: &mut EventIter,
) -> Option<Definition> {
    let mut port = Definition::default();
    let mut tokens = String::new();
    match node {
        AnsiPortDeclaration::Net(x) => {
            eprintln!("found ansi_port_net");
            let ident = get_ident(tree, RefNode::PortIdentifier(&x.nodes.1));
            port.ident = ident.0;
            port.byte_idx = ident.1;
            match &x.nodes.0 {
                Some(y) => match y {
                    NetPortHeaderOrInterfacePortHeader::InterfacePortHeader(z) => match &**z {
                        InterfacePortHeader::Identifier(node) => {
                            port.interface = Some(
                                get_ident(tree, RefNode::InterfaceIdentifier(&node.nodes.0)).0,
                            );
                            match &node.nodes.1 {
                                Some((_, mod_ident)) => {
                                    port.modport = Some(
                                        get_ident(tree, RefNode::ModportIdentifier(mod_ident)).0,
                                    );
                                }
                                None => (),
                            }
                        }
                        InterfacePortHeader::Interface(node) => {
                            port.interface = Some("interface".to_string());
                            match &node.nodes.1 {
                                Some((_, mod_ident)) => {
                                    port.modport = Some(
                                        get_ident(tree, RefNode::ModportIdentifier(mod_ident)).0,
                                    );
                                }
                                None => (),
                            }
                        }
                    },
                    _ => (),
                },
                None => (),
            }
        }
        _ => (),
    }

    advance_until_leave!(tokens, tree, event_iter, RefNode::AnsiPortDeclaration);
    port.type_str = tokens;
    port.kind = CompletionItemKind::Property;
    Some(port)
}

fn list_port_idents(
    tree: &SyntaxTree,
    node: &ListOfPortIdentifiers,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut ports: Vec<Definition> = Vec::new();
    let mut port_list = vec![&node.nodes.0.nodes.0];
    for port_def in &node.nodes.0.nodes.1 {
        port_list.push(&port_def.1);
    }
    for port_def in port_list {
        let mut port = Definition::default();
        let ident = get_ident(tree, RefNode::PortIdentifier(&port_def.0));
        port.ident = ident.0;
        port.byte_idx = ident.1;
        for unpacked_dim in &port_def.1 {
            let tokens = &mut port.type_str;
            advance_until_leave!(tokens, tree, event_iter, RefNode::UnpackedDimension);
        }
        ports.push(port);
    }
    Some(ports)
}

fn list_interface_idents(
    tree: &SyntaxTree,
    node: &ListOfInterfaceIdentifiers,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut ports: Vec<Definition> = Vec::new();
    let mut port_list = vec![&node.nodes.0.nodes.0];
    for port_def in &node.nodes.0.nodes.1 {
        port_list.push(&port_def.1);
    }
    for port_def in port_list {
        let mut port = Definition::default();
        let ident = get_ident(tree, RefNode::InterfaceIdentifier(&port_def.0));
        port.ident = ident.0;
        port.byte_idx = ident.1;
        for unpacked_dim in &port_def.1 {
            let tokens = &mut port.type_str;
            advance_until_leave!(tokens, tree, event_iter, RefNode::UnpackedDimension);
        }
        ports.push(port);
    }
    Some(ports)
}

fn list_variable_idents(
    tree: &SyntaxTree,
    node: &ListOfVariableIdentifiers,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut ports: Vec<Definition> = Vec::new();
    let mut port_list = vec![&node.nodes.0.nodes.0];
    for port_def in &node.nodes.0.nodes.1 {
        port_list.push(&port_def.1);
    }
    for port_def in port_list {
        let mut port = Definition::default();
        let ident = get_ident(tree, RefNode::VariableIdentifier(&port_def.0));
        port.ident = ident.0;
        port.byte_idx = ident.1;
        for variable_dim in &port_def.1 {
            let tokens = &mut port.type_str;
            advance_until_leave!(tokens, tree, event_iter, RefNode::VariableDimension);
        }
        ports.push(port);
    }
    Some(ports)
}

fn port_dec_non_ansi(
    tree: &SyntaxTree,
    node: &PortDeclaration,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut ports: Vec<Definition>;
    let mut common = String::new();
    eprintln!("found non-ansi ports");
    match node {
        PortDeclaration::Inout(x) => {
            let port_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfPortIdentifiers,
                &ListOfPortIdentifiers
            )?;
            ports = list_port_idents(tree, &port_list, event_iter)?;
        }
        PortDeclaration::Input(x) => match &x.nodes.1 {
            InputDeclaration::Net(y) => {
                let port_list = advance_until_enter!(
                    common,
                    tree,
                    event_iter,
                    RefNode::ListOfPortIdentifiers,
                    &ListOfPortIdentifiers
                )?;
                ports = list_port_idents(tree, &port_list, event_iter)?;
            }
            InputDeclaration::Variable(y) => {
                let port_list = advance_until_enter!(
                    common,
                    tree,
                    event_iter,
                    RefNode::ListOfVariableIdentifiers,
                    &ListOfVariableIdentifiers
                )?;
                ports = list_variable_idents(tree, &port_list, event_iter)?;
            }
        },
        PortDeclaration::Output(x) => match &x.nodes.1 {
            OutputDeclaration::Net(y) => {
                let port_list = advance_until_enter!(
                    common,
                    tree,
                    event_iter,
                    RefNode::ListOfPortIdentifiers,
                    &ListOfPortIdentifiers
                )?;
                ports = list_port_idents(tree, &port_list, event_iter)?;
            }
            OutputDeclaration::Variable(y) => {
                let port_list = advance_until_enter!(
                    common,
                    tree,
                    event_iter,
                    RefNode::ListOfVariableIdentifiers,
                    &ListOfVariableIdentifiers
                )?;
                ports = list_variable_idents(tree, &port_list, event_iter)?;
            }
        },
        PortDeclaration::Ref(x) => {
            let port_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfVariableIdentifiers,
                &ListOfVariableIdentifiers
            )?;
            ports = list_variable_idents(tree, &port_list, event_iter)?;
        }
        PortDeclaration::Interface(x) => {
            let interface =
                Some(get_ident(tree, RefNode::InterfaceIdentifier(&x.nodes.1.nodes.0)).0);
            let modport = match &x.nodes.1.nodes.1 {
                Some((_, mod_ident)) => {
                    Some(get_ident(tree, RefNode::ModportIdentifier(mod_ident)).0)
                }
                None => None,
            };
            let port_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfInterfaceIdentifiers,
                &ListOfInterfaceIdentifiers
            )?;
            ports = list_interface_idents(tree, &port_list, event_iter)?;
            for port in &mut ports {
                port.interface = interface.clone();
                port.modport = modport.clone();
            }
        }
    }
    for port in &mut ports {
        port.type_str = format!("{} {}", common, port.type_str);
        port.kind = CompletionItemKind::Property;
    }
    Some(ports)
}

fn list_net_decl(
    tree: &SyntaxTree,
    node: &ListOfNetDeclAssignments,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut nets: Vec<Definition> = Vec::new();
    let mut net_list = vec![&node.nodes.0.nodes.0];
    for net_def in &node.nodes.0.nodes.1 {
        net_list.push(&net_def.1);
    }
    for net_def in net_list {
        let mut net = Definition::default();
        let ident = get_ident(tree, RefNode::NetIdentifier(&net_def.nodes.0));
        net.ident = ident.0;
        net.byte_idx = ident.1;
        for variable_dim in &net_def.nodes.1 {
            let tokens = &mut net.type_str;
            advance_until_leave!(tokens, tree, event_iter, RefNode::UnpackedDimension);
        }
        nets.push(net);
    }
    Some(nets)
}

fn net_dec(
    tree: &SyntaxTree,
    node: &NetDeclaration,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut nets: Vec<Definition>;
    let mut common = String::new();
    eprintln!("found net");
    match node {
        NetDeclaration::NetType(x) => {
            let net_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfNetDeclAssignments,
                &ListOfNetDeclAssignments
            )?;
            nets = list_net_decl(tree, net_list, event_iter)?;
        }
        NetDeclaration::NetTypeIdentifier(x) => {
            let net_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfNetDeclAssignments,
                &ListOfNetDeclAssignments
            )?;
            nets = list_net_decl(tree, net_list, event_iter)?;
        }
        NetDeclaration::Interconnect(x) => {
            let mut net = Definition::default();
            let ident = get_ident(tree, RefNode::NetIdentifier(&x.nodes.3));
            net.ident = ident.0;
            net.byte_idx = ident.1;
            advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::NetIdentifier,
                &NetIdentifier
            );
            for unpacked_dim in &x.nodes.4 {
                advance_until_leave!(common, tree, event_iter, RefNode::UnpackedDimension);
            }
            nets = vec![net];
        }
    }
    for net in &mut nets {
        net.type_str = format!("{} {}", common, net.type_str);
        net.kind = CompletionItemKind::Variable;
    }
    Some(nets)
}

fn list_var_decl(
    tree: &SyntaxTree,
    node: &ListOfVariableDeclAssignments,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut vars: Vec<Definition> = Vec::new();
    let mut var_list = vec![&node.nodes.0.nodes.0];
    for var_def in &node.nodes.0.nodes.1 {
        var_list.push(&var_def.1);
    }
    for var_def in var_list {
        let mut var = Definition::default();
        match &var_def {
            VariableDeclAssignment::Variable(node) => {
                let ident = get_ident(tree, RefNode::VariableIdentifier(&node.nodes.0));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                for variable_dim in &node.nodes.1 {
                    let tokens = &mut var.type_str;
                    advance_until_leave!(tokens, tree, event_iter, RefNode::VariableDimension);
                }
            }
            VariableDeclAssignment::DynamicArray(node) => {
                let ident = get_ident(tree, RefNode::DynamicArrayVariableIdentifier(&node.nodes.0));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                for variable_dim in &node.nodes.2 {
                    let tokens = &mut var.type_str;
                    advance_until_leave!(tokens, tree, event_iter, RefNode::VariableDimension);
                }
            }
            VariableDeclAssignment::Class(node) => {
                let ident = get_ident(tree, RefNode::ClassVariableIdentifier(&node.nodes.0));
                var.ident = ident.0;
                var.byte_idx = ident.1;
            }
        }
        vars.push(var);
    }
    Some(vars)
}

fn data_dec(
    tree: &SyntaxTree,
    node: &DataDeclaration,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut data: Vec<Definition>;
    let mut common = String::new();
    eprintln!("found data_dec");
    match node {
        DataDeclaration::Variable(x) => {
            let var_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfVariableDeclAssignments,
                &ListOfVariableDeclAssignments
            )?;
            data = list_var_decl(tree, var_list, event_iter)?;
        }
        DataDeclaration::TypeDeclaration(x) => match &**x {
            TypeDeclaration::DataType(y) => {
                let mut var = Definition::default();
                let ident = get_ident(tree, RefNode::TypeIdentifier(&y.nodes.2));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                for variable_dim in &y.nodes.3 {
                    let tokens = &mut var.type_str;
                    advance_until_leave!(tokens, tree, event_iter, RefNode::VariableDimension);
                }
                data = vec![var];
            }
            TypeDeclaration::Interface(y) => {
                let mut var = Definition::default();
                let ident = get_ident(tree, RefNode::TypeIdentifier(&y.nodes.5));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                let mut tokens = String::new();
                advance_until_enter!(
                    tokens,
                    tree,
                    event_iter,
                    RefNode::TypeIdentifier,
                    &TypeIdentifier
                );
                advance_until_enter!(
                    tokens,
                    tree,
                    event_iter,
                    RefNode::TypeIdentifier,
                    &TypeIdentifier
                );
                var.type_str = tokens;
                data = vec![var];
            }
            TypeDeclaration::Reserved(y) => {
                let mut var = Definition::default();
                let ident = get_ident(tree, RefNode::TypeIdentifier(&y.nodes.2));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                let mut tokens = String::new();
                advance_until_enter!(
                    tokens,
                    tree,
                    event_iter,
                    RefNode::TypeIdentifier,
                    &TypeIdentifier
                );
                var.type_str = tokens;
                data = vec![var];
            }
        },
        DataDeclaration::PackageImportDeclaration(x) => {
            let mut import_list = vec![&x.nodes.1.nodes.0];
            for import_def in &x.nodes.1.nodes.1 {
                import_list.push(&import_def.1);
            }
            data = Vec::new();
            for import_def in import_list {
                let mut import = Definition::default();
                match import_def {
                    PackageImportItem::Identifier(y) => {
                        let ident = get_ident(tree, RefNode::PackageIdentifier(&y.nodes.0));
                        import.ident = ident.0;
                        import.byte_idx = ident.1;
                        let import_loc = match &y.nodes.2 {
                            Identifier::SimpleIdentifier(id) => id.nodes.0,
                            Identifier::EscapedIdentifier(id) => id.nodes.0,
                        };
                        import.import_ident = Some(tree.get_str(&import_loc)?.to_string());
                    }
                    PackageImportItem::Asterisk(y) => {
                        let ident = get_ident(tree, RefNode::PackageIdentifier(&y.nodes.0));
                        import.ident = ident.0;
                        import.byte_idx = ident.1;
                    }
                }
                data.push(import);
            }
        }
        DataDeclaration::NetTypeDeclaration(x) => match &**x {
            NetTypeDeclaration::DataType(y) => {
                let mut var = Definition::default();
                let ident = get_ident(tree, RefNode::NetTypeIdentifier(&y.nodes.2));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                let mut tokens = String::new();
                advance_until_enter!(
                    tokens,
                    tree,
                    event_iter,
                    RefNode::NetTypeIdentifier,
                    &NetTypeIdentifier
                );
                var.type_str = tokens;
                data = vec![var];
            }
            NetTypeDeclaration::NetType(y) => {
                let mut var = Definition::default();
                let ident = get_ident(tree, RefNode::NetTypeIdentifier(&y.nodes.2));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                let mut tokens = String::new();
                advance_until_leave!(tokens, tree, event_iter, RefNode::NetTypeIdentifier);
                var.type_str = tokens;
                data = vec![var];
            }
        },
    }
    for var in &mut data {
        var.type_str = format!("{} {}", common, var.type_str);
        var.kind = CompletionItemKind::Variable;
    }
    Some(data)
}

fn tfport_list(
    tree: &SyntaxTree,
    node: &TfPortList,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut tfportss: Vec<Definition> = Vec::new();
    let mut tfports_list = vec![&node.nodes.0.nodes.0];
    for tfports_def in &node.nodes.0.nodes.1 {
        tfports_list.push(&tfports_def.1);
    }
    for tfports_def in tfports_list {
        match &tfports_def.nodes.4 {
            Some(def) => {
                let mut tfports = Definition::default();
                let ident = get_ident(tree, RefNode::PortIdentifier(&def.0));
                tfports.ident = ident.0;
                tfports.byte_idx = ident.1;
                tfports.kind = CompletionItemKind::Property;
                for variable_dim in &def.1 {
                    let tokens = &mut tfports.type_str;
                    advance_until_leave!(tokens, tree, event_iter, RefNode::UnpackedDimension);
                }
                tfportss.push(tfports);
            }
            None => (),
        }
    }
    Some(tfportss)
}

fn function_dec(
    tree: &SyntaxTree,
    node: &FunctionDeclaration,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut defs: Vec<Definition>;
    eprintln!("found func");
    match &node.nodes.2 {
        FunctionBodyDeclaration::WithoutPort(x) => {
            let mut func = Definition::default();
            let ident = get_ident(tree, RefNode::FunctionIdentifier(&x.nodes.2));
            func.ident = ident.0;
            func.byte_idx = ident.1;
            let mut tokens = String::new();
            advance_until_enter!(
                tokens,
                tree,
                event_iter,
                RefNode::FunctionIdentifier,
                &FunctionIdentifier
            );
            func.type_str = tokens;
            func.kind = CompletionItemKind::Function;
            defs = vec![func];
        }
        FunctionBodyDeclaration::WithPort(x) => {
            let mut func = Definition::default();
            let ident = get_ident(tree, RefNode::FunctionIdentifier(&x.nodes.2));
            func.ident = ident.0;
            func.byte_idx = ident.1;
            let mut tokens = String::new();
            advance_until_enter!(
                tokens,
                tree,
                event_iter,
                RefNode::FunctionIdentifier,
                &FunctionIdentifier
            );
            func.type_str = tokens;
            func.kind = CompletionItemKind::Function;
            defs = vec![func];
            match &x.nodes.3.nodes.1 {
                Some(tfports) => {
                    skip_until_enter!(tree, event_iter, RefNode::TfPortList, &TfPortList);
                    let mut ports = tfport_list(tree, tfports, event_iter)?;
                    defs.append(&mut ports);
                }
                None => (),
            }
        }
    }
    Some(defs)
}

fn task_dec(
    tree: &SyntaxTree,
    node: &TaskDeclaration,
    event_iter: &mut EventIter,
) -> Option<Vec<Definition>> {
    let mut defs: Vec<Definition>;
    eprintln!("found task");
    match &node.nodes.2 {
        TaskBodyDeclaration::WithoutPort(x) => {
            let mut task = Definition::default();
            let ident = get_ident(tree, RefNode::TaskIdentifier(&x.nodes.1));
            task.ident = ident.0;
            task.byte_idx = ident.1;
            let mut tokens = String::new();
            advance_until_enter!(
                tokens,
                tree,
                event_iter,
                RefNode::TaskIdentifier,
                &TaskIdentifier
            );
            task.type_str = tokens;
            task.kind = CompletionItemKind::Function;
            defs = vec![task];
        }
        TaskBodyDeclaration::WithPort(x) => {
            let mut task = Definition::default();
            let ident = get_ident(tree, RefNode::TaskIdentifier(&x.nodes.1));
            task.ident = ident.0;
            task.byte_idx = ident.1;
            let mut tokens = String::new();
            advance_until_enter!(
                tokens,
                tree,
                event_iter,
                RefNode::TaskIdentifier,
                &TaskIdentifier
            );
            task.type_str = tokens;
            task.kind = CompletionItemKind::Function;
            defs = vec![task];
            match &x.nodes.2.nodes.1 {
                Some(tfports) => {
                    skip_until_enter!(tree, event_iter, RefNode::TfPortList, &TfPortList);
                    let mut ports = tfport_list(tree, tfports, event_iter)?;
                    defs.append(&mut ports);
                }
                None => (),
            }
        }
    }
    Some(defs)
}

pub fn get_definitions(
    syntax_tree: &SyntaxTree,
    scope_idents: &Vec<(String, usize, usize)>,
) -> Option<Vec<Definition>> {
    eprintln!("{}", syntax_tree);

    let mut definitions = Vec::new();
    let mut event_iter = syntax_tree.into_iter().event();
    while let Some(event) = event_iter.next() {
        match event {
            NodeEvent::Enter(node) => match node {
                RefNode::AnsiPortDeclaration(n) => {
                    let port = port_dec_ansi(syntax_tree, n, &mut event_iter);
                    if port.is_some() {
                        definitions.push(port?);
                    }
                }
                RefNode::PortDeclaration(n) => {
                    let port = port_dec_non_ansi(syntax_tree, n, &mut event_iter);
                    if port.is_some() {
                        definitions.append(&mut port?);
                    }
                }
                RefNode::NetDeclaration(n) => {
                    let nets = net_dec(syntax_tree, n, &mut event_iter);
                    if nets.is_some() {
                        definitions.append(&mut nets?);
                    }
                }
                RefNode::DataDeclaration(n) => {
                    let vars = data_dec(syntax_tree, n, &mut event_iter);
                    if vars.is_some() {
                        definitions.append(&mut vars?);
                    }
                }
                RefNode::FunctionDeclaration(n) => {
                    let decs = function_dec(syntax_tree, n, &mut event_iter);
                    if decs.is_some() {
                        definitions.append(&mut decs?);
                    }
                }
                RefNode::TaskDeclaration(n) => {
                    let decs = task_dec(syntax_tree, n, &mut event_iter);
                    if decs.is_some() {
                        definitions.append(&mut decs?);
                    }
                }
                _ => (),
            },
            NodeEvent::Leave(_) => (),
        }
    }
    for def in &mut definitions {
        def.type_str = clean_type_str(&def.type_str, &def.ident);
    }
    Some(definitions)
}

fn get_hover(doc: &Rope, line: usize) -> String {
    if line == 0 {
        return doc.line(line).to_string();
    }
    let mut hover: Vec<String> = Vec::new();
    let mut multiline: bool = false;
    let mut valid: bool = true;
    let mut current: String = doc.line(line).to_string();
    let ltrim: String = " ".repeat(current.len() - current.trim_start().len());
    let mut line_idx = line;
    while valid {
        hover.push(current.clone());
        line_idx -= 1;
        valid = false;
        if line_idx > 0 {
            current = doc.line(line_idx).to_string();
            let currentl = current.clone().trim_start().to_owned();
            let currentr = current.clone().trim_end().to_owned();
            if currentl.starts_with("/*") && currentr.ends_with("*/") {
                valid = true;
            } else if currentr.ends_with("*/") {
                multiline = true;
                valid = true;
            } else if currentl.starts_with("/*") {
                multiline = false;
                valid = true;
            } else if currentl.starts_with("//") {
                valid = true;
            } else if multiline {
                valid = true;
            } else {
                valid = false;
            }
        }
    }
    hover.reverse();
    let mut result: Vec<String> = Vec::new();
    for i in hover {
        if let Some(stripped) = i.strip_prefix(&ltrim) {
            result.push(stripped.to_owned());
        } else {
            result.push(i);
        }
    }
    result.join("").trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::completion::get_scope_idents;
    use crate::sources::{parse, LSPSupport};
    use ropey::Rope;
    use std::collections::HashMap;
    use std::fs::read_to_string;
    use std::path::PathBuf;

    #[test]
    fn test_definition_token() {
        let line = Rope::from_str("assign ab_c[2:0] = 3'b000;");
        let token = get_definition_token(line.line(0), Position::new(0, 10));
        assert_eq!(token, "ab_c".to_owned());
    }

    #[test]
    fn test_get_definition() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("tests_rtl/definition_test.sv");
        let text = read_to_string(d).unwrap();
        let doc = Rope::from_str(&text);
        let syntax_tree = parse(
            &doc.clone(),
            &Url::parse("file:///tests_rtl/definition_test.sv").unwrap(),
            &None,
        )
        .unwrap();
        let scope_idents = get_scope_idents(&syntax_tree);
        let defs = get_definitions(&syntax_tree, &scope_idents).unwrap();
        for def in &defs {
            println!("{:?} {:?}", def, doc.byte_to_pos(def.byte_idx));
        }
        /*
        let token = get_definition_token(doc.line(3), Position::new(3, 13));
        for def in defs {
            if token == def.ident {
                assert_eq!(doc.byte_to_pos(def.byte_idx), Position::new(3, 9))
            }
        }
        */
        assert!(false);
    }

    #[test]
    fn test_hover() {
        let text = r#"
// module test
// test module
module test;
  /* a */
  logic a;
  /**
    * b
  */
  logic b;
  endmodule"#;
        let doc = Rope::from_str(text);
        eprintln!("{}", get_hover(&doc, 2));
        assert_eq!(
            get_hover(&doc, 3),
            r#"// module test
// test module
module test;"#
                .to_owned()
        );
        assert_eq!(
            get_hover(&doc, 9),
            r#"/**
  * b
*/
logic b;"#
                .to_owned()
        );
    }
}
