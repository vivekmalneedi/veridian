use crate::definition::def_types::*;
use crate::definition::match_definitions;
use sv_parser::*;
use tower_lsp::lsp_types::*;

fn get_ident(tree: &SyntaxTree, node: RefNode) -> (String, usize) {
    let loc = unwrap_locate!(node).unwrap();
    let ident_str = tree.get_str(loc).unwrap().to_string();
    let byte_idx = tree.get_origin(loc).unwrap().1;
    (ident_str, byte_idx)
}

fn get_loc(tree: &SyntaxTree, node: RefNode) -> usize {
    let loc = unwrap_locate!(node).unwrap();
    tree.get_origin(loc).unwrap().1
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

macro_rules! match_until_leave {
    ($tree:ident, $event_iter:ident, $url:ident, $node:path) => {{
        let mut scopes: Vec<Box<dyn Scope>> = Vec::new();
        let mut definitions: Vec<Box<dyn Definition>> = Vec::new();
        let mut global_scope: GenericScope = GenericScope::new($url);
        global_scope.ident = "global".to_string();
        while let Some(event) = $event_iter.next() {
            match event {
                NodeEvent::Enter(node) => {
                    let mut result = match_definitions($tree, $event_iter, node, $url)?;
                    definitions.append(&mut result.1);
                    scopes.append(&mut result.0);
                }
                NodeEvent::Leave(node) => match node {
                    $node(n) => {
                        break;
                    }
                    _ => {}
                },
            }
        }
        Some((scopes, definitions))
    }};
}

pub fn port_dec_ansi(
    tree: &SyntaxTree,
    node: &AnsiPortDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<PortDec> {
    let mut port = PortDec::new(url);
    let mut tokens = String::new();
    if let AnsiPortDeclaration::Net(x) = node {
        let ident = get_ident(tree, RefNode::PortIdentifier(&x.nodes.1));
        port.ident = ident.0;
        port.byte_idx = ident.1;
        if let Some(y) = &x.nodes.0 {
            if let NetPortHeaderOrInterfacePortHeader::InterfacePortHeader(z) = y {
                match &**z {
                    InterfacePortHeader::Identifier(node) => {
                        port.interface =
                            Some(get_ident(tree, RefNode::InterfaceIdentifier(&node.nodes.0)).0);
                        match &node.nodes.1 {
                            Some((_, mod_ident)) => {
                                port.modport =
                                    Some(get_ident(tree, RefNode::ModportIdentifier(mod_ident)).0);
                            }
                            None => (),
                        }
                    }
                    InterfacePortHeader::Interface(node) => {
                        port.interface = Some("interface".to_string());
                        match &node.nodes.1 {
                            Some((_, mod_ident)) => {
                                port.modport =
                                    Some(get_ident(tree, RefNode::ModportIdentifier(mod_ident)).0);
                            }
                            None => (),
                        }
                    }
                }
            }
        }
    }

    advance_until_leave!(tokens, tree, event_iter, RefNode::AnsiPortDeclaration);
    port.type_str = tokens;
    Some(port)
}

pub fn list_port_idents(
    tree: &SyntaxTree,
    node: &ListOfPortIdentifiers,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PortDec>> {
    let mut ports: Vec<PortDec> = Vec::new();
    let mut port_list = vec![&node.nodes.0.nodes.0];
    for port_def in &node.nodes.0.nodes.1 {
        port_list.push(&port_def.1);
    }
    for port_def in port_list {
        let mut port = PortDec::new(url);
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

pub fn list_interface_idents(
    tree: &SyntaxTree,
    node: &ListOfInterfaceIdentifiers,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PortDec>> {
    let mut ports: Vec<PortDec> = Vec::new();
    let mut port_list = vec![&node.nodes.0.nodes.0];
    for port_def in &node.nodes.0.nodes.1 {
        port_list.push(&port_def.1);
    }
    for port_def in port_list {
        let mut port = PortDec::new(url);
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

pub fn list_variable_idents(
    tree: &SyntaxTree,
    node: &ListOfVariableIdentifiers,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PortDec>> {
    let mut ports: Vec<PortDec> = Vec::new();
    let mut port_list = vec![&node.nodes.0.nodes.0];
    for port_def in &node.nodes.0.nodes.1 {
        port_list.push(&port_def.1);
    }
    for port_def in port_list {
        let mut port = PortDec::new(url);
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

pub fn port_dec_non_ansi(
    tree: &SyntaxTree,
    node: &PortDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PortDec>> {
    let mut ports: Vec<PortDec>;
    let mut common = String::new();
    match node {
        PortDeclaration::Inout(x) => {
            let port_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfPortIdentifiers,
                &ListOfPortIdentifiers
            )?;
            ports = list_port_idents(tree, &port_list, event_iter, url)?;
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
                ports = list_port_idents(tree, &port_list, event_iter, url)?;
            }
            InputDeclaration::Variable(y) => {
                let port_list = advance_until_enter!(
                    common,
                    tree,
                    event_iter,
                    RefNode::ListOfVariableIdentifiers,
                    &ListOfVariableIdentifiers
                )?;
                ports = list_variable_idents(tree, &port_list, event_iter, url)?;
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
                ports = list_port_idents(tree, &port_list, event_iter, url)?;
            }
            OutputDeclaration::Variable(y) => {
                let port_list = advance_until_enter!(
                    common,
                    tree,
                    event_iter,
                    RefNode::ListOfVariableIdentifiers,
                    &ListOfVariableIdentifiers
                )?;
                ports = list_variable_idents(tree, &port_list, event_iter, url)?;
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
            ports = list_variable_idents(tree, &port_list, event_iter, url)?;
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
            ports = list_interface_idents(tree, &port_list, event_iter, url)?;
            for port in &mut ports {
                port.interface = interface.clone();
                port.modport = modport.clone();
            }
        }
    }
    for port in &mut ports {
        port.type_str = format!("{} {}", common, port.type_str);
    }
    Some(ports)
}

pub fn list_net_decl(
    tree: &SyntaxTree,
    node: &ListOfNetDeclAssignments,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    let mut nets: Vec<GenericDec> = Vec::new();
    let mut net_list = vec![&node.nodes.0.nodes.0];
    for net_def in &node.nodes.0.nodes.1 {
        net_list.push(&net_def.1);
    }
    for net_def in net_list {
        let mut net = GenericDec::new(url);
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

pub fn net_dec(
    tree: &SyntaxTree,
    node: &NetDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    let mut nets: Vec<GenericDec>;
    let mut common = String::new();
    match node {
        NetDeclaration::NetType(x) => {
            let net_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfNetDeclAssignments,
                &ListOfNetDeclAssignments
            )?;
            nets = list_net_decl(tree, net_list, event_iter, url)?;
        }
        NetDeclaration::NetTypeIdentifier(x) => {
            let net_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfNetDeclAssignments,
                &ListOfNetDeclAssignments
            )?;
            nets = list_net_decl(tree, net_list, event_iter, url)?;
        }
        NetDeclaration::Interconnect(x) => {
            let mut net = GenericDec::new(url);
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
        net.completion_kind = CompletionItemKind::Variable;
        net.symbol_kind = SymbolKind::Variable;
        net.type_str = format!("{} {}", common, net.type_str);
    }
    Some(nets)
}

pub fn list_var_decl(
    tree: &SyntaxTree,
    node: &ListOfVariableDeclAssignments,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    let mut vars: Vec<GenericDec> = Vec::new();
    let mut var_list = vec![&node.nodes.0.nodes.0];
    for var_def in &node.nodes.0.nodes.1 {
        var_list.push(&var_def.1);
    }
    for var_def in var_list {
        let mut var = GenericDec::new(url);
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

pub fn package_import(
    tree: &SyntaxTree,
    node: &PackageImportDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PackageImport>> {
    let mut import_list = vec![&node.nodes.1.nodes.0];
    for import_def in &node.nodes.1.nodes.1 {
        import_list.push(&import_def.1);
    }
    let mut imports = Vec::new();
    for import_def in import_list {
        let mut import = PackageImport::new(url);
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
                import.asterisk = true;
            }
        }
        imports.push(import);
    }
    Some(imports)
}

pub fn data_dec(
    tree: &SyntaxTree,
    node: &DataDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<Box<dyn Definition>>> {
    let mut data: Vec<Box<dyn Definition>>;
    let mut common = String::new();
    match node {
        DataDeclaration::Variable(x) => {
            let var_list = advance_until_enter!(
                common,
                tree,
                event_iter,
                RefNode::ListOfVariableDeclAssignments,
                &ListOfVariableDeclAssignments
            )?;
            let mut decs = list_var_decl(tree, var_list, event_iter, url)?;
            data = Vec::new();
            for var in &mut decs {
                var.type_str = format!("{} {}", common, var.type_str);
                var.completion_kind = CompletionItemKind::Variable;
                var.symbol_kind = SymbolKind::Variable;
            }
            for var in decs {
                data.push(Box::new(var));
            }
        }
        DataDeclaration::TypeDeclaration(x) => match &**x {
            TypeDeclaration::DataType(y) => {
                let mut var = GenericDec::new(url);
                let ident = get_ident(tree, RefNode::TypeIdentifier(&y.nodes.2));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                for variable_dim in &y.nodes.3 {
                    let tokens = &mut var.type_str;
                    advance_until_leave!(tokens, tree, event_iter, RefNode::VariableDimension);
                }
                var.type_str = format!("{} {}", common, var.type_str);
                var.completion_kind = CompletionItemKind::Variable;
                var.symbol_kind = SymbolKind::Variable;
                data = vec![Box::new(var)];
            }
            TypeDeclaration::Interface(y) => {
                let mut var = GenericDec::new(url);
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
                var.type_str = format!("{} {}", common, var.type_str);
                var.completion_kind = CompletionItemKind::Interface;
                var.symbol_kind = SymbolKind::Interface;
                data = vec![Box::new(var)];
            }
            TypeDeclaration::Reserved(y) => {
                let mut var = GenericDec::new(url);
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
                var.type_str = format!("{} {}", common, var.type_str);
                var.completion_kind = CompletionItemKind::Variable;
                var.symbol_kind = SymbolKind::Variable;
                data = vec![Box::new(var)];
            }
        },
        DataDeclaration::PackageImportDeclaration(x) => {
            data = Vec::new();
            let imports = package_import(tree, x, event_iter, url)?;
            for import in imports {
                data.push(Box::new(import));
            }
        }
        DataDeclaration::NetTypeDeclaration(x) => match &**x {
            NetTypeDeclaration::DataType(y) => {
                let mut var = GenericDec::new(url);
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
                var.type_str = format!("{} {}", common, var.type_str);
                var.completion_kind = CompletionItemKind::Variable;
                var.symbol_kind = SymbolKind::Variable;
                data = vec![Box::new(var)];
            }
            NetTypeDeclaration::NetType(y) => {
                let mut var = GenericDec::new(url);
                let ident = get_ident(tree, RefNode::NetTypeIdentifier(&y.nodes.2));
                var.ident = ident.0;
                var.byte_idx = ident.1;
                let mut tokens = String::new();
                advance_until_leave!(tokens, tree, event_iter, RefNode::NetTypeIdentifier);
                var.type_str = tokens;
                var.type_str = format!("{} {}", common, var.type_str);
                var.completion_kind = CompletionItemKind::Variable;
                var.symbol_kind = SymbolKind::Variable;
                data = vec![Box::new(var)];
            }
        },
    }
    Some(data)
}

pub fn tfport_list(
    tree: &SyntaxTree,
    node: &TfPortList,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PortDec>> {
    let mut tfports: Vec<PortDec> = Vec::new();
    let mut tfports_list = vec![&node.nodes.0.nodes.0];
    for tfports_def in &node.nodes.0.nodes.1 {
        tfports_list.push(&tfports_def.1);
    }
    for tfports_def in tfports_list {
        match &tfports_def.nodes.4 {
            Some(def) => {
                let mut tfport = PortDec::new(url);
                let ident = get_ident(tree, RefNode::PortIdentifier(&def.0));
                tfport.ident = ident.0;
                tfport.byte_idx = ident.1;
                for variable_dim in &def.1 {
                    let tokens = &mut tfport.type_str;
                    advance_until_leave!(tokens, tree, event_iter, RefNode::UnpackedDimension);
                }
                tfports.push(tfport);
            }
            None => (),
        }
    }
    Some(tfports)
}

pub fn function_dec(
    tree: &SyntaxTree,
    node: &FunctionDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<SubDec> {
    let mut func: SubDec = SubDec::new(url);
    func.start = get_loc(tree, RefNode::Keyword(&node.nodes.0));
    match &node.nodes.2 {
        FunctionBodyDeclaration::WithoutPort(x) => {
            func.end = get_loc(tree, RefNode::Keyword(&x.nodes.6));
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
        }
        FunctionBodyDeclaration::WithPort(x) => {
            func.end = get_loc(tree, RefNode::Keyword(&x.nodes.7));
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
            match &x.nodes.3.nodes.1 {
                Some(tfports) => {
                    skip_until_enter!(tree, event_iter, RefNode::TfPortList, &TfPortList);
                    let ports = tfport_list(tree, tfports, event_iter, url)?;
                    for port in ports {
                        func.defs.push(Box::new(port));
                    }
                }
                None => (),
            }
        }
    }
    let (scopes, mut defs) =
        match_until_leave!(tree, event_iter, url, RefNode::FunctionDeclaration)?;
    func.scopes = scopes;
    func.defs.append(&mut defs);
    Some(func)
}

pub fn task_dec(
    tree: &SyntaxTree,
    node: &TaskDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<SubDec> {
    let mut task = SubDec::new(url);
    task.start = get_loc(tree, RefNode::Keyword(&node.nodes.0));
    match &node.nodes.2 {
        TaskBodyDeclaration::WithoutPort(x) => {
            task.end = get_loc(tree, RefNode::Keyword(&x.nodes.5));
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
        }
        TaskBodyDeclaration::WithPort(x) => {
            task.end = get_loc(tree, RefNode::Keyword(&x.nodes.6));
            let mut task = SubDec::new(url);
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
            match &x.nodes.2.nodes.1 {
                Some(tfports) => {
                    skip_until_enter!(tree, event_iter, RefNode::TfPortList, &TfPortList);
                    let ports = tfport_list(tree, tfports, event_iter, url)?;
                    for port in ports {
                        task.defs.push(Box::new(port));
                    }
                }
                None => (),
            }
        }
    }
    let (scopes, mut defs) = match_until_leave!(tree, event_iter, url, RefNode::TaskDeclaration)?;
    task.scopes = scopes;
    task.defs.append(&mut defs);
    Some(task)
}

pub fn modport_dec(
    tree: &SyntaxTree,
    node: &ModportDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<ModportDec>> {
    let mut modports: Vec<ModportDec> = Vec::new();
    let mut common = String::new();
    advance_until_enter!(common, tree, event_iter, RefNode::ModportItem, &ModportItem);
    let mut modports_list = vec![&node.nodes.1.nodes.0];
    for modports_def in &node.nodes.1.nodes.1 {
        modports_list.push(&modports_def.1);
    }
    for modport_def in modports_list {
        let mut modport = ModportDec::new(url);
        let ident = get_ident(tree, RefNode::ModportIdentifier(&modport_def.nodes.0));
        modport.ident = ident.0;
        modport.byte_idx = ident.1;
        modport.type_str = common.clone();

        let mut mp_port_decs = vec![&modport_def.nodes.1.nodes.1.nodes.0];
        for mp_port_def in &modport_def.nodes.1.nodes.1.nodes.1 {
            mp_port_decs.push(&mp_port_def.1);
        }
        for mp_port_dec in mp_port_decs {
            match mp_port_dec {
                ModportPortsDeclaration::Simple(x) => {
                    skip_until_enter!(
                        tree,
                        event_iter,
                        RefNode::ModportPortsDeclarationSimple,
                        &ModportPortsDeclarationSimple
                    );
                    let mut prepend = String::new();
                    advance_until_enter!(
                        prepend,
                        tree,
                        event_iter,
                        RefNode::ModportSimplePort,
                        &ModportSimplePort
                    );
                    let mut mp_simple_port_decs = vec![&x.nodes.1.nodes.1.nodes.0];
                    for mp_simple_dec in &x.nodes.1.nodes.1.nodes.1 {
                        mp_simple_port_decs.push(&mp_simple_dec.1);
                    }
                    for mp_simple_def in mp_simple_port_decs {
                        match mp_simple_def {
                            ModportSimplePort::Ordered(y) => {
                                let mut port = PortDec::new(url);
                                let ident = get_ident(tree, RefNode::PortIdentifier(&y.nodes.0));
                                port.ident = ident.0;
                                port.byte_idx = ident.1;
                                port.type_str = prepend.clone();
                                modport.ports.push(Box::new(port));
                            }
                            ModportSimplePort::Named(y) => {
                                let port_ident = skip_until_enter!(
                                    tree,
                                    event_iter,
                                    RefNode::PortIdentifier,
                                    &PortIdentifier
                                )?;
                                let mut port = PortDec::new(url);
                                let ident = get_ident(tree, RefNode::PortIdentifier(port_ident));
                                port.ident = ident.0;
                                port.byte_idx = ident.1;
                                let mut append = String::new();
                                advance_until_leave!(
                                    append,
                                    tree,
                                    event_iter,
                                    RefNode::ModportSimplePortNamed
                                );
                                port.type_str = format!("{} {}", prepend, append);
                                modport.ports.push(Box::new(port));
                            }
                        }
                    }
                }
                ModportPortsDeclaration::Tf(x) => {
                    skip_until_enter!(
                        tree,
                        event_iter,
                        RefNode::ModportPortsDeclarationTf,
                        &ModportPortsDeclarationTf
                    );
                    let mut prepend = String::new();
                    let mp_tf_ports_dec = advance_until_enter!(
                        prepend,
                        tree,
                        event_iter,
                        RefNode::ModportTfPortsDeclaration,
                        &ModportTfPortsDeclaration
                    )?;
                    let mut mp_tf_ports = vec![&mp_tf_ports_dec.nodes.1.nodes.0];
                    for mp_tf_port_dec in &mp_tf_ports_dec.nodes.1.nodes.1 {
                        mp_tf_ports.push(&mp_tf_port_dec.1);
                    }
                    for mp_tf_port in mp_tf_ports {
                        match mp_tf_port {
                            ModportTfPort::MethodPrototype(y) => match &**y {
                                MethodPrototype::TaskPrototype(z) => {
                                    let mut port = SubDec::new(url);
                                    let ident =
                                        get_ident(tree, RefNode::TaskIdentifier(&z.nodes.1));
                                    port.ident = ident.0;
                                    port.byte_idx = ident.1;
                                    skip_until_enter!(
                                        tree,
                                        event_iter,
                                        RefNode::TaskPrototype,
                                        &TaskPrototype
                                    );
                                    let tokens = &mut port.type_str;
                                    advance_until_leave!(
                                        tokens,
                                        tree,
                                        event_iter,
                                        RefNode::TaskPrototype
                                    );
                                    modport.ports.push(Box::new(port));
                                }
                                MethodPrototype::FunctionPrototype(z) => {
                                    let mut port = SubDec::new(url);
                                    let ident =
                                        get_ident(tree, RefNode::FunctionIdentifier(&z.nodes.2));
                                    port.ident = ident.0;
                                    port.byte_idx = ident.1;
                                    skip_until_enter!(
                                        tree,
                                        event_iter,
                                        RefNode::FunctionPrototype,
                                        &FunctionPrototype
                                    );
                                    let tokens = &mut port.type_str;
                                    advance_until_leave!(
                                        tokens,
                                        tree,
                                        event_iter,
                                        RefNode::FunctionIdentifier
                                    );
                                    modport.ports.push(Box::new(port));
                                }
                            },
                            ModportTfPort::TfIdentifier(y) => {
                                let mut port = SubDec::new(url);
                                let ident = get_ident(tree, RefNode::TfIdentifier(&y));
                                port.ident = ident.0;
                                port.byte_idx = ident.1;
                                port.type_str = prepend.clone();
                                modport.ports.push(Box::new(port));
                            }
                        }
                    }
                }
                ModportPortsDeclaration::Clocking(x) => {
                    skip_until_enter!(
                        tree,
                        event_iter,
                        RefNode::ModportPortsDeclarationClocking,
                        &ModportPortsDeclarationClocking
                    );
                    let mut tokens = String::new();
                    let clock_ident = advance_until_enter!(
                        tokens,
                        tree,
                        event_iter,
                        RefNode::ClockingIdentifier,
                        &ClockingIdentifier
                    )?;
                    let ident = get_ident(tree, RefNode::ClockingIdentifier(clock_ident));
                    let mut port = PortDec::new(url);
                    port.ident = ident.0;
                    port.byte_idx = ident.1;
                    port.type_str = tokens;
                    modport.ports.push(Box::new(port));
                }
            }
        }

        modports.push(modport);
    }
    Some(modports)
}

pub fn module_inst(
    tree: &SyntaxTree,
    node: &ModuleInstantiation,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<ModInst>> {
    let mut defs: Vec<ModInst> = Vec::new();
    let mod_ident = get_ident(tree, RefNode::ModuleIdentifier(&node.nodes.0)).0;
    let mut instances = vec![&node.nodes.2.nodes.0];
    for inst in &node.nodes.2.nodes.1 {
        instances.push(&inst.1);
    }
    for inst in instances {
        let hinst = skip_until_enter!(
            tree,
            event_iter,
            RefNode::HierarchicalInstance,
            &HierarchicalInstance
        )?;
        let mut instance = ModInst::new(url);
        let ident = get_ident(tree, RefNode::InstanceIdentifier(&hinst.nodes.0.nodes.0));
        instance.ident = ident.0;
        instance.byte_idx = ident.1;
        instance.type_str = mod_ident.clone();
        instance.mod_ident = mod_ident.clone();
        let type_str = &mut instance.type_str;
        for unpacked_dim in &hinst.nodes.0.nodes.1 {
            advance_until_leave!(type_str, tree, event_iter, RefNode::UnpackedDimension);
        }
        defs.push(instance);
    }
    Some(defs)
}

fn param_assignment(
    tree: &SyntaxTree,
    node: &ParamAssignment,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericDec> {
    let param_assign =
        skip_until_enter!(tree, event_iter, RefNode::ParamAssignment, &ParamAssignment)?;
    let mut def = GenericDec::new(url);
    let ident = get_ident(tree, RefNode::ParameterIdentifier(&param_assign.nodes.0));
    def.ident = ident.0;
    def.byte_idx = ident.1;
    let type_str = &mut def.type_str;
    def.completion_kind = CompletionItemKind::TypeParameter;
    def.symbol_kind = SymbolKind::TypeParameter;
    advance_until_leave!(type_str, tree, event_iter, RefNode::ParamAssignment);
    Some(def)
}

fn list_param_assignment(
    tree: &SyntaxTree,
    node: &ListOfParamAssignments,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    let mut defs: Vec<GenericDec> = Vec::new();
    let p_a_list = skip_until_enter!(
        tree,
        event_iter,
        RefNode::ListOfParamAssignments,
        &ListOfParamAssignments
    )?;
    let mut param_assigns = vec![&p_a_list.nodes.0.nodes.0];
    for param_assign in &p_a_list.nodes.0.nodes.1 {
        param_assigns.push(&param_assign.1);
    }
    for param_assign in param_assigns {
        defs.push(param_assignment(tree, param_assign, event_iter, url)?);
    }
    Some(defs)
}

fn type_assignment(
    tree: &SyntaxTree,
    node: &TypeAssignment,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericDec> {
    let type_assign =
        skip_until_enter!(tree, event_iter, RefNode::TypeAssignment, &TypeAssignment)?;
    let mut def = GenericDec::new(url);
    let ident = get_ident(tree, RefNode::TypeIdentifier(&type_assign.nodes.0));
    def.ident = ident.0;
    def.byte_idx = ident.1;
    def.completion_kind = CompletionItemKind::TypeParameter;
    def.symbol_kind = SymbolKind::TypeParameter;
    let type_str = &mut def.type_str;
    advance_until_leave!(type_str, tree, event_iter, RefNode::TypeAssignment);
    Some(def)
}

fn list_type_assignment(
    tree: &SyntaxTree,
    node: &ListOfTypeAssignments,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    let mut defs: Vec<GenericDec> = Vec::new();
    let p_a_list = skip_until_enter!(
        tree,
        event_iter,
        RefNode::ListOfTypeAssignments,
        &ListOfTypeAssignments
    )?;
    let mut type_assigns = vec![&p_a_list.nodes.0.nodes.0];
    for type_assign in &p_a_list.nodes.0.nodes.1 {
        type_assigns.push(&type_assign.1);
    }
    for type_assign in type_assigns {
        defs.push(type_assignment(tree, type_assign, event_iter, url)?);
    }
    Some(defs)
}

fn param_dec(
    tree: &SyntaxTree,
    node: &ParameterDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    let param_dec = skip_until_enter!(
        tree,
        event_iter,
        RefNode::ParameterDeclaration,
        &ParameterDeclaration
    )?;
    match param_dec {
        ParameterDeclaration::Param(x) => {
            let mut prepend = String::new();
            advance_until_leave!(prepend, tree, event_iter, RefNode::DataTypeOrImplicit);
            let mut defs = list_param_assignment(tree, &x.nodes.2, event_iter, url)?;
            for def in &mut defs {
                def.type_str = format!("{} {}", prepend, def.type_str);
                def.completion_kind = CompletionItemKind::TypeParameter;
                def.symbol_kind = SymbolKind::TypeParameter;
            }
            Some(defs)
        }
        ParameterDeclaration::Type(x) => {
            let mut prepend = String::new();
            advance_until_leave!(prepend, tree, event_iter, RefNode::Keyword);
            advance_until_leave!(prepend, tree, event_iter, RefNode::Keyword);
            let mut defs = list_type_assignment(tree, &x.nodes.2, event_iter, url)?;
            for def in &mut defs {
                def.type_str = format!("{} {}", prepend, def.type_str);
                def.completion_kind = CompletionItemKind::TypeParameter;
                def.symbol_kind = SymbolKind::TypeParameter;
            }
            Some(defs)
        }
    }
}

fn localparam_dec(
    tree: &SyntaxTree,
    node: &LocalParameterDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    let localparam_dec = skip_until_enter!(
        tree,
        event_iter,
        RefNode::LocalParameterDeclaration,
        &LocalParameterDeclaration
    )?;
    match localparam_dec {
        LocalParameterDeclaration::Param(x) => {
            let mut prepend = String::new();
            advance_until_leave!(prepend, tree, event_iter, RefNode::DataTypeOrImplicit);
            let mut defs = list_param_assignment(tree, &x.nodes.2, event_iter, url)?;
            for def in &mut defs {
                def.type_str = format!("{} {}", prepend, def.type_str);
                def.completion_kind = CompletionItemKind::TypeParameter;
                def.symbol_kind = SymbolKind::TypeParameter;
            }
            Some(defs)
        }
        LocalParameterDeclaration::Type(x) => {
            let mut prepend = String::new();
            advance_until_leave!(prepend, tree, event_iter, RefNode::Keyword);
            advance_until_leave!(prepend, tree, event_iter, RefNode::Keyword);
            let mut defs = list_type_assignment(tree, &x.nodes.2, event_iter, url)?;
            for def in &mut defs {
                def.type_str = format!("{} {}", prepend, def.type_str);
                def.completion_kind = CompletionItemKind::TypeParameter;
                def.symbol_kind = SymbolKind::TypeParameter;
            }
            Some(defs)
        }
    }
}

fn param_port_dec(
    tree: &SyntaxTree,
    node: &ParameterPortDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    match node {
        ParameterPortDeclaration::ParameterDeclaration(x) => param_dec(tree, x, event_iter, url),
        ParameterPortDeclaration::LocalParameterDeclaration(x) => {
            localparam_dec(tree, x, event_iter, url)
        }
        ParameterPortDeclaration::ParamList(x) => {
            let mut prepend = String::new();
            advance_until_leave!(prepend, tree, event_iter, RefNode::DataType);
            let mut defs = list_param_assignment(tree, &x.nodes.1, event_iter, url)?;
            for def in &mut defs {
                def.type_str = format!("{} {}", prepend, def.type_str);
                def.completion_kind = CompletionItemKind::TypeParameter;
                def.symbol_kind = SymbolKind::TypeParameter;
            }
            Some(defs)
        }
        ParameterPortDeclaration::TypeList(x) => {
            let mut prepend = String::new();
            advance_until_leave!(prepend, tree, event_iter, RefNode::Keyword);
            let mut defs = list_type_assignment(tree, &x.nodes.1, event_iter, url)?;
            for def in &mut defs {
                def.type_str = format!("{} {}", prepend, def.type_str);
                def.completion_kind = CompletionItemKind::TypeParameter;
                def.symbol_kind = SymbolKind::TypeParameter;
            }
            Some(defs)
        }
    }
}

pub fn param_port_list(
    tree: &SyntaxTree,
    node: &ParameterPortList,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<GenericDec>> {
    let mut defs: Vec<GenericDec> = Vec::new();
    match node {
        ParameterPortList::Assignment(x) => {
            defs.append(&mut list_param_assignment(
                tree,
                &(x.nodes.1.nodes.1).0,
                event_iter,
                url,
            )?);
            for port_dec in &(x.nodes.1.nodes.1).1 {
                defs.append(&mut param_port_dec(tree, &port_dec.1, event_iter, url)?);
            }
        }
        ParameterPortList::Declaration(x) => {
            let mut param_port_decs = vec![&x.nodes.1.nodes.1.nodes.0];
            for param_port_dec in &x.nodes.1.nodes.1.nodes.1 {
                param_port_decs.push(&param_port_dec.1);
            }
            for port_dec in param_port_decs {
                defs.append(&mut param_port_dec(tree, port_dec, event_iter, url)?);
            }
        }
        ParameterPortList::Empty(_) => {}
    }
    Some(defs)
}

pub fn module_dec(
    tree: &SyntaxTree,
    node: &ModuleDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericScope> {
    let mut scope: GenericScope = GenericScope::new(url);
    match node {
        ModuleDeclaration::Nonansi(x) => {
            scope.start = get_loc(tree, RefNode::ModuleKeyword(&x.nodes.0.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.3));
            let ident = get_ident(tree, RefNode::ModuleIdentifier(&x.nodes.0.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ModuleIdentifier);
            for import_dec in &x.nodes.0.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.0.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
        }
        ModuleDeclaration::Ansi(x) => {
            scope.start = get_loc(tree, RefNode::ModuleKeyword(&x.nodes.0.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.3));
            let ident = get_ident(tree, RefNode::ModuleIdentifier(&x.nodes.0.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ModuleIdentifier);
            for import_dec in &x.nodes.0.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.0.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
            if let Some(list_port_decs) = &x.nodes.0.nodes.6 {
                if let Some(port_decs) = &list_port_decs.nodes.0.nodes.1 {
                    let mut port_decs_list: Vec<&AnsiPortDeclaration> =
                        vec![&(port_decs.nodes.0).1];
                    for port_dec in &port_decs.nodes.1 {
                        port_decs_list.push(&(port_dec.1).1);
                    }
                    for port_dec in port_decs_list {
                        let ansi_dec = skip_until_enter!(
                            tree,
                            event_iter,
                            RefNode::AnsiPortDeclaration,
                            &AnsiPortDeclaration
                        )?;
                        scope
                            .defs
                            .push(Box::new(port_dec_ansi(tree, ansi_dec, event_iter, url)?))
                    }
                }
            }
        }
        ModuleDeclaration::Wildcard(x) => {
            scope.start = get_loc(tree, RefNode::ModuleKeyword(&x.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.8));
            let ident = get_ident(tree, RefNode::ModuleIdentifier(&x.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ModuleIdentifier);
        }
        ModuleDeclaration::ExternNonansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0));
            scope.end = get_loc(tree, RefNode::Symbol(&x.nodes.1.nodes.7));
            let ident = get_ident(tree, RefNode::ModuleIdentifier(&x.nodes.1.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ModuleIdentifier);
            for import_dec in &x.nodes.1.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.1.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
        }
        ModuleDeclaration::ExternAnsi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0));
            scope.end = get_loc(tree, RefNode::Symbol(&x.nodes.1.nodes.7));
            let ident = get_ident(tree, RefNode::ModuleIdentifier(&x.nodes.1.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ModuleIdentifier);
            for import_dec in &x.nodes.1.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.1.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
            if let Some(list_port_decs) = &x.nodes.1.nodes.6 {
                if let Some(port_decs) = &list_port_decs.nodes.0.nodes.1 {
                    let mut port_decs_list: Vec<&AnsiPortDeclaration> =
                        vec![&(port_decs.nodes.0).1];
                    for port_dec in &port_decs.nodes.1 {
                        port_decs_list.push(&(port_dec.1).1);
                    }
                    for port_dec in port_decs_list {
                        let ansi_dec = skip_until_enter!(
                            tree,
                            event_iter,
                            RefNode::AnsiPortDeclaration,
                            &AnsiPortDeclaration
                        )?;
                        scope
                            .defs
                            .push(Box::new(port_dec_ansi(tree, ansi_dec, event_iter, url)?))
                    }
                }
            }
        }
    }
    let (scopes, mut defs) = match_until_leave!(tree, event_iter, url, RefNode::ModuleDeclaration)?;
    scope.scopes = scopes;
    scope.defs.append(&mut defs);
    scope.completion_kind = CompletionItemKind::Module;
    scope.symbol_kind = SymbolKind::Module;
    Some(scope)
}

pub fn interface_dec(
    tree: &SyntaxTree,
    node: &InterfaceDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericScope> {
    let mut scope: GenericScope = GenericScope::new(url);
    match node {
        InterfaceDeclaration::Nonansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.3));
            let ident = get_ident(tree, RefNode::InterfaceIdentifier(&x.nodes.0.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::InterfaceIdentifier);
            for import_dec in &x.nodes.0.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.0.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
        }
        InterfaceDeclaration::Ansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.3));
            let ident = get_ident(tree, RefNode::InterfaceIdentifier(&x.nodes.0.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::InterfaceIdentifier);
            for import_dec in &x.nodes.0.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.0.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
            if let Some(list_port_decs) = &x.nodes.0.nodes.6 {
                if let Some(port_decs) = &list_port_decs.nodes.0.nodes.1 {
                    let mut port_decs_list: Vec<&AnsiPortDeclaration> =
                        vec![&(port_decs.nodes.0).1];
                    for port_dec in &port_decs.nodes.1 {
                        port_decs_list.push(&(port_dec.1).1);
                    }
                    for port_dec in port_decs_list {
                        let ansi_dec = skip_until_enter!(
                            tree,
                            event_iter,
                            RefNode::AnsiPortDeclaration,
                            &AnsiPortDeclaration
                        )?;
                        scope
                            .defs
                            .push(Box::new(port_dec_ansi(tree, ansi_dec, event_iter, url)?))
                    }
                }
            }
        }
        InterfaceDeclaration::Wildcard(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.8));
            let ident = get_ident(tree, RefNode::InterfaceIdentifier(&x.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::InterfaceIdentifier);
        }
        InterfaceDeclaration::ExternNonansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0));
            scope.end = get_loc(tree, RefNode::Symbol(&x.nodes.1.nodes.7));
            let ident = get_ident(tree, RefNode::InterfaceIdentifier(&x.nodes.1.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::InterfaceIdentifier);
            for import_dec in &x.nodes.1.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.1.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
        }
        InterfaceDeclaration::ExternAnsi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0));
            scope.end = get_loc(tree, RefNode::Symbol(&x.nodes.1.nodes.7));
            let ident = get_ident(tree, RefNode::InterfaceIdentifier(&x.nodes.1.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::InterfaceIdentifier);
            for import_dec in &x.nodes.1.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.1.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
            if let Some(list_port_decs) = &x.nodes.1.nodes.6 {
                if let Some(port_decs) = &list_port_decs.nodes.0.nodes.1 {
                    let mut port_decs_list: Vec<&AnsiPortDeclaration> =
                        vec![&(port_decs.nodes.0).1];
                    for port_dec in &port_decs.nodes.1 {
                        port_decs_list.push(&(port_dec.1).1);
                    }
                    for port_dec in port_decs_list {
                        let ansi_dec = skip_until_enter!(
                            tree,
                            event_iter,
                            RefNode::AnsiPortDeclaration,
                            &AnsiPortDeclaration
                        )?;
                        scope
                            .defs
                            .push(Box::new(port_dec_ansi(tree, ansi_dec, event_iter, url)?))
                    }
                }
            }
        }
    }
    let (scopes, mut defs) =
        match_until_leave!(tree, event_iter, url, RefNode::InterfaceDeclaration)?;
    scope.scopes = scopes;
    scope.defs.append(&mut defs);
    scope.completion_kind = CompletionItemKind::Interface;
    scope.symbol_kind = SymbolKind::Interface;
    Some(scope)
}

fn list_udp_port_idents(
    tree: &SyntaxTree,
    node: &ListOfUdpPortIdentifiers,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PortDec>> {
    let mut ports: Vec<PortDec> = Vec::new();
    let mut port_list = vec![&node.nodes.0.nodes.0];
    for port_def in &node.nodes.0.nodes.1 {
        port_list.push(&port_def.1);
    }
    for port_def in port_list {
        let mut port = PortDec::new(url);
        let ident = get_ident(tree, RefNode::PortIdentifier(&port_def));
        port.ident = ident.0;
        port.byte_idx = ident.1;
        ports.push(port);
    }
    Some(ports)
}

//non-ansi udp ports
fn udp_port_dec(
    tree: &SyntaxTree,
    node: &UdpPortDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PortDec>> {
    match node {
        UdpPortDeclaration::UdpOutputDeclaration(x) => match &x.0 {
            UdpOutputDeclaration::Nonreg(x) => {
                let mut port = PortDec::new(url);
                let ident = get_ident(tree, RefNode::PortIdentifier(&x.nodes.2));
                port.ident = ident.0;
                port.byte_idx = ident.1;
                skip_until_enter!(
                    tree,
                    event_iter,
                    RefNode::UdpOutputDeclarationNonreg,
                    &UdpOutputDeclarationNonreg
                );
                let type_str = &mut port.type_str;
                advance_until_leave!(
                    type_str,
                    tree,
                    event_iter,
                    RefNode::UdpOutputDeclarationNonreg
                );
                Some(vec![port])
            }
            UdpOutputDeclaration::Reg(x) => {
                let mut port = PortDec::new(url);
                let ident = get_ident(tree, RefNode::PortIdentifier(&x.nodes.3));
                port.ident = ident.0;
                port.byte_idx = ident.1;
                skip_until_enter!(
                    tree,
                    event_iter,
                    RefNode::UdpOutputDeclarationReg,
                    &UdpOutputDeclarationReg
                );
                let type_str = &mut port.type_str;
                advance_until_leave!(type_str, tree, event_iter, RefNode::UdpOutputDeclarationReg);
                Some(vec![port])
            }
        },
        UdpPortDeclaration::UdpInputDeclaration(_) => {
            skip_until_enter!(
                tree,
                event_iter,
                RefNode::UdpInputDeclaration,
                &UdpInputDeclaration
            );
            let mut type_str = String::new();
            let list_udp_ports = advance_until_enter!(
                type_str,
                tree,
                event_iter,
                RefNode::ListOfUdpPortIdentifiers,
                &ListOfUdpPortIdentifiers
            )?;
            let mut ports = list_udp_port_idents(tree, list_udp_ports, event_iter, url)?;
            for port in &mut ports {
                port.type_str = type_str.clone();
            }
            Some(ports)
        }
        UdpPortDeclaration::UdpRegDeclaration(x) => {
            let udp_reg_dec = skip_until_enter!(
                tree,
                event_iter,
                RefNode::UdpRegDeclaration,
                &UdpRegDeclaration
            )?;
            let mut port = PortDec::new(url);
            let type_str = &mut port.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::Keyword);
            let ident = get_ident(tree, RefNode::VariableIdentifier(&udp_reg_dec.nodes.2));
            port.ident = ident.0;
            port.byte_idx = ident.1;
            Some(vec![port])
        }
    }
}

//ansi udp ports
fn udp_port_list(
    tree: &SyntaxTree,
    node: &UdpDeclarationPortList,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<PortDec>> {
    let mut ports: Vec<PortDec> = Vec::new();
    match &node.nodes.0 {
        UdpOutputDeclaration::Nonreg(x) => {
            let mut port = PortDec::new(url);
            let ident = get_ident(tree, RefNode::PortIdentifier(&x.nodes.2));
            port.ident = ident.0;
            port.byte_idx = ident.1;
            skip_until_enter!(
                tree,
                event_iter,
                RefNode::UdpOutputDeclarationNonreg,
                &UdpOutputDeclarationNonreg
            );
            let type_str = &mut port.type_str;
            advance_until_leave!(
                type_str,
                tree,
                event_iter,
                RefNode::UdpOutputDeclarationNonreg
            );
            ports.push(port);
        }
        UdpOutputDeclaration::Reg(x) => {
            let mut port = PortDec::new(url);
            let ident = get_ident(tree, RefNode::PortIdentifier(&x.nodes.3));
            port.ident = ident.0;
            port.byte_idx = ident.1;
            skip_until_enter!(
                tree,
                event_iter,
                RefNode::UdpOutputDeclarationReg,
                &UdpOutputDeclarationReg
            );
            let type_str = &mut port.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::UdpOutputDeclarationReg);
            ports.push(port);
        }
    }
    let mut port_list = vec![&node.nodes.2.nodes.0];
    for port_def in &node.nodes.2.nodes.1 {
        port_list.push(&port_def.1);
    }
    for port_def in port_list {
        skip_until_enter!(
            tree,
            event_iter,
            RefNode::UdpInputDeclaration,
            &UdpInputDeclaration
        );
        let mut type_str = String::new();
        let list_udp_ports = advance_until_enter!(
            type_str,
            tree,
            event_iter,
            RefNode::ListOfUdpPortIdentifiers,
            &ListOfUdpPortIdentifiers
        )?;
        let mut port_decs = list_udp_port_idents(tree, list_udp_ports, event_iter, url)?;
        for port in &mut port_decs {
            port.type_str = type_str.clone();
        }
        ports.append(&mut port_decs);
    }
    Some(ports)
}

pub fn udp_dec(
    tree: &SyntaxTree,
    node: &UdpDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericScope> {
    let mut scope: GenericScope = GenericScope::new(url);
    match node {
        UdpDeclaration::Nonansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.4));
            let ident = get_ident(tree, RefNode::UdpIdentifier(&x.nodes.0.nodes.2));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::UdpIdentifier);
            let mut port_decs = vec![&x.nodes.1];
            for port_dec in &x.nodes.2 {
                port_decs.push(port_dec);
            }
            for port in port_decs {
                let ports = udp_port_dec(tree, port, event_iter, url)?;
                for port_dec in ports {
                    scope.defs.push(Box::new(port_dec));
                }
            }
        }
        UdpDeclaration::Ansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.2));
            let ident = get_ident(tree, RefNode::UdpIdentifier(&x.nodes.0.nodes.2));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::UdpIdentifier);
            let ports = udp_port_list(tree, &x.nodes.0.nodes.3.nodes.1, event_iter, url)?;
            for port_dec in ports {
                scope.defs.push(Box::new(port_dec));
            }
        }
        UdpDeclaration::ExternNonansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0));
            scope.end = get_loc(tree, RefNode::Symbol(&x.nodes.1.nodes.4));
            let ident = get_ident(tree, RefNode::UdpIdentifier(&x.nodes.1.nodes.2));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::UdpIdentifier);
        }
        UdpDeclaration::ExternAnsi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0));
            scope.end = get_loc(tree, RefNode::Symbol(&x.nodes.1.nodes.4));
            let ident = get_ident(tree, RefNode::UdpIdentifier(&x.nodes.1.nodes.2));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::UdpIdentifier);
            let ports = udp_port_list(tree, &x.nodes.1.nodes.3.nodes.1, event_iter, url)?;
            for port_dec in ports {
                scope.defs.push(Box::new(port_dec));
            }
        }
        UdpDeclaration::Wildcard(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.7));
            let ident = get_ident(tree, RefNode::UdpIdentifier(&x.nodes.2));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::UdpIdentifier);
            for port_dec in &x.nodes.5 {
                let ports = udp_port_dec(tree, port_dec, event_iter, url)?;
                for port in ports {
                    scope.defs.push(Box::new(port));
                }
            }
        }
    }

    let (scopes, mut defs) = match_until_leave!(tree, event_iter, url, RefNode::UdpDeclaration)?;
    scope.scopes = scopes;
    scope.defs.append(&mut defs);
    scope.completion_kind = CompletionItemKind::Module;
    scope.symbol_kind = SymbolKind::Module;
    Some(scope)
}

pub fn program_dec(
    tree: &SyntaxTree,
    node: &ProgramDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericScope> {
    let mut scope: GenericScope = GenericScope::new(url);
    match node {
        ProgramDeclaration::Nonansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.3));
            let ident = get_ident(tree, RefNode::ProgramIdentifier(&x.nodes.0.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::InterfaceIdentifier);
            for import_dec in &x.nodes.0.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.0.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
        }

        ProgramDeclaration::Ansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.3));
            let ident = get_ident(tree, RefNode::ProgramIdentifier(&x.nodes.0.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ProgramIdentifier);
            for import_dec in &x.nodes.0.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.0.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
            if let Some(list_port_decs) = &x.nodes.0.nodes.6 {
                if let Some(port_decs) = &list_port_decs.nodes.0.nodes.1 {
                    let mut port_decs_list: Vec<&AnsiPortDeclaration> =
                        vec![&(port_decs.nodes.0).1];
                    for port_dec in &port_decs.nodes.1 {
                        port_decs_list.push(&(port_dec.1).1);
                    }
                    for port_dec in port_decs_list {
                        let ansi_dec = skip_until_enter!(
                            tree,
                            event_iter,
                            RefNode::AnsiPortDeclaration,
                            &AnsiPortDeclaration
                        )?;
                        scope
                            .defs
                            .push(Box::new(port_dec_ansi(tree, ansi_dec, event_iter, url)?))
                    }
                }
            }
        }
        ProgramDeclaration::Wildcard(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.1));
            scope.end = get_loc(tree, RefNode::Keyword(&x.nodes.7));
            let ident = get_ident(tree, RefNode::ProgramIdentifier(&x.nodes.2));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ProgramIdentifier);
        }
        ProgramDeclaration::ExternNonansi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0));
            scope.end = get_loc(tree, RefNode::Symbol(&x.nodes.1.nodes.7));
            let ident = get_ident(tree, RefNode::ProgramIdentifier(&x.nodes.1.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ProgramIdentifier);
            for import_dec in &x.nodes.1.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.1.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
        }
        ProgramDeclaration::ExternAnsi(x) => {
            scope.start = get_loc(tree, RefNode::Keyword(&x.nodes.0));
            scope.end = get_loc(tree, RefNode::Symbol(&x.nodes.1.nodes.7));
            let ident = get_ident(tree, RefNode::ProgramIdentifier(&x.nodes.1.nodes.3));
            scope.ident = ident.0;
            scope.byte_idx = ident.1;
            let type_str = &mut scope.type_str;
            advance_until_leave!(type_str, tree, event_iter, RefNode::ProgramIdentifier);
            for import_dec in &x.nodes.1.nodes.4 {
                let imports = package_import(tree, &import_dec, event_iter, url)?;
                for import in imports {
                    scope.defs.push(Box::new(import));
                }
            }
            if let Some(pport_list) = &x.nodes.1.nodes.5 {
                let pports = param_port_list(tree, pport_list, event_iter, url)?;
                for pport in pports {
                    scope.defs.push(Box::new(pport));
                }
            }
            if let Some(list_port_decs) = &x.nodes.1.nodes.6 {
                if let Some(port_decs) = &list_port_decs.nodes.0.nodes.1 {
                    let mut port_decs_list: Vec<&AnsiPortDeclaration> =
                        vec![&(port_decs.nodes.0).1];
                    for port_dec in &port_decs.nodes.1 {
                        port_decs_list.push(&(port_dec.1).1);
                    }
                    for port_dec in port_decs_list {
                        let ansi_dec = skip_until_enter!(
                            tree,
                            event_iter,
                            RefNode::AnsiPortDeclaration,
                            &AnsiPortDeclaration
                        )?;
                        scope
                            .defs
                            .push(Box::new(port_dec_ansi(tree, ansi_dec, event_iter, url)?))
                    }
                }
            }
        }
    }

    let (scopes, mut defs) =
        match_until_leave!(tree, event_iter, url, RefNode::ProgramDeclaration)?;
    scope.scopes = scopes;
    scope.defs.append(&mut defs);
    scope.completion_kind = CompletionItemKind::Module;
    scope.symbol_kind = SymbolKind::Module;
    Some(scope)
}

pub fn package_dec(
    tree: &SyntaxTree,
    node: &PackageDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericScope> {
    let mut scope: GenericScope = GenericScope::new(url);
    scope.start = get_loc(tree, RefNode::Keyword(&node.nodes.1));
    scope.end = get_loc(tree, RefNode::Keyword(&node.nodes.7));
    let ident = get_ident(tree, RefNode::PackageIdentifier(&node.nodes.3));
    scope.ident = ident.0;
    scope.byte_idx = ident.1;
    let type_str = &mut scope.type_str;
    advance_until_leave!(type_str, tree, event_iter, RefNode::PackageIdentifier);

    let (scopes, mut defs) =
        match_until_leave!(tree, event_iter, url, RefNode::PackageDeclaration)?;
    scope.scopes = scopes;
    scope.defs.append(&mut defs);
    scope.completion_kind = CompletionItemKind::Module;
    scope.symbol_kind = SymbolKind::Package;
    Some(scope)
}

pub fn config_dec(
    tree: &SyntaxTree,
    node: &ConfigDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericScope> {
    let mut scope: GenericScope = GenericScope::new(url);
    scope.start = get_loc(tree, RefNode::Keyword(&node.nodes.0));
    scope.end = get_loc(tree, RefNode::Keyword(&node.nodes.6));
    let ident = get_ident(tree, RefNode::ConfigIdentifier(&node.nodes.1));
    scope.ident = ident.0;
    scope.byte_idx = ident.1;
    let type_str = &mut scope.type_str;
    advance_until_leave!(type_str, tree, event_iter, RefNode::ConfigIdentifier);
    for localparam in &node.nodes.3 {
        let params = localparam_dec(tree, &localparam.0, event_iter, url)?;
        for param in params {
            scope.defs.push(Box::new(param));
        }
    }

    let (scopes, mut defs) = match_until_leave!(tree, event_iter, url, RefNode::ConfigDeclaration)?;
    scope.scopes = scopes;
    scope.defs.append(&mut defs);
    scope.completion_kind = CompletionItemKind::Module;
    scope.symbol_kind = SymbolKind::Module;
    Some(scope)
}

pub fn class_dec(
    tree: &SyntaxTree,
    node: &ClassDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<ClassDec> {
    let mut scope: ClassDec = ClassDec::new(url);
    scope.start = get_loc(tree, RefNode::Keyword(&node.nodes.1));
    scope.end = get_loc(tree, RefNode::Keyword(&node.nodes.9));
    let ident = get_ident(tree, RefNode::ClassIdentifier(&node.nodes.3));
    scope.ident = ident.0;
    scope.byte_idx = ident.1;
    let type_str = &mut scope.type_str;
    advance_until_leave!(type_str, tree, event_iter, RefNode::ClassIdentifier);
    if let Some(pport_list) = &node.nodes.4 {
        let pports = param_port_list(tree, pport_list, event_iter, url)?;
        for pport in pports {
            scope.defs.push(Box::new(pport));
        }
    }
    if let Some(extend) = &node.nodes.5 {
        if let Some(package_scope) = &extend.1.nodes.0.nodes.0 {
            match package_scope {
                PackageScope::Package(x) => {
                    let ident = get_ident(tree, RefNode::PackageIdentifier(&x.nodes.0));
                    scope.extends.1 = Some(ident.0);
                }
                PackageScope::Unit(_) => {}
            }
        }
        let ident = get_ident(tree, RefNode::ClassIdentifier(&extend.1.nodes.0.nodes.1));
        scope.extends.0.push(ident.0);
        for class in &extend.1.nodes.2 {
            let ident = get_ident(tree, RefNode::ClassIdentifier(&class.1));
            scope.extends.0.push(ident.0);
        }
    }
    if let Some(interfaces) = &node.nodes.6 {
        let mut idecs = vec![&interfaces.1.nodes.0];
        for idec in &interfaces.1.nodes.1 {
            idecs.push(&idec.1);
        }
        for idec in idecs {
            let ident = get_ident(tree, RefNode::ClassIdentifier(&idec.nodes.0.nodes.1));
            let mut interface: (String, Option<String>) = (ident.0, None);
            if let Some(package_scope) = &idec.nodes.0.nodes.0 {
                match package_scope {
                    PackageScope::Package(x) => {
                        let ident = get_ident(tree, RefNode::PackageIdentifier(&x.nodes.0));
                        interface.1 = Some(ident.0);
                    }
                    PackageScope::Unit(_) => {}
                }
            }
            scope.implements.push(interface);
        }
    }

    let (scopes, mut defs) = match_until_leave!(tree, event_iter, url, RefNode::ClassDeclaration)?;
    scope.scopes = scopes;
    scope.defs.append(&mut defs);
    Some(scope)
}

// `define definition
pub fn text_macro_def(
    tree: &SyntaxTree,
    node: &TextMacroDefinition,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<GenericDec> {
    let mut text_macro = GenericDec::new(url);
    let ident = get_ident(tree, RefNode::TextMacroIdentifier(&node.nodes.2.nodes.0));
    text_macro.ident = ident.0;
    text_macro.byte_idx = ident.1;
    let type_str = &mut text_macro.type_str;
    advance_until_enter!(
        type_str,
        tree,
        event_iter,
        RefNode::TextMacroIdentifier,
        &TextMacroIdentifier
    );
    text_macro.completion_kind = CompletionItemKind::Function;
    text_macro.symbol_kind = SymbolKind::Function;
    Some(text_macro)
}
