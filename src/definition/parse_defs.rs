use crate::definition::def_types::*;
use std::sync::Arc;
use sv_parser::*;
use tower_lsp::lsp_types::*;

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

pub fn port_dec_ansi(
    tree: &SyntaxTree,
    node: &AnsiPortDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<PortDec> {
    let mut port = PortDec::new(url);
    let mut tokens = String::new();
    match node {
        AnsiPortDeclaration::Net(x) => {
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
        port.kind = CompletionItemKind::Property;
    }
    Some(ports)
}

pub fn list_net_decl(
    tree: &SyntaxTree,
    node: &ListOfNetDeclAssignments,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<NetDec>> {
    let mut nets: Vec<NetDec> = Vec::new();
    let mut net_list = vec![&node.nodes.0.nodes.0];
    for net_def in &node.nodes.0.nodes.1 {
        net_list.push(&net_def.1);
    }
    for net_def in net_list {
        let mut net = NetDec::new(url);
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
) -> Option<Vec<NetDec>> {
    let mut nets: Vec<NetDec>;
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
            let mut net = NetDec::new(url);
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
    }
    Some(nets)
}

pub fn list_var_decl(
    tree: &SyntaxTree,
    node: &ListOfVariableDeclAssignments,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<DataDec>> {
    let mut vars: Vec<DataDec> = Vec::new();
    let mut var_list = vec![&node.nodes.0.nodes.0];
    for var_def in &node.nodes.0.nodes.1 {
        var_list.push(&var_def.1);
    }
    for var_def in var_list {
        let mut var = DataDec::new(url);
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

pub fn data_dec(
    tree: &SyntaxTree,
    node: &DataDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<DataDec>> {
    let mut data: Vec<DataDec>;
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
            data = list_var_decl(tree, var_list, event_iter, url)?;
        }
        DataDeclaration::TypeDeclaration(x) => match &**x {
            TypeDeclaration::DataType(y) => {
                let mut var = DataDec::new(url);
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
                let mut var = DataDec::new(url);
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
                let mut var = DataDec::new(url);
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
                let mut import = DataDec::new(url);
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
                let mut var = DataDec::new(url);
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
                let mut var = DataDec::new(url);
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
                tfport.kind = CompletionItemKind::Property;
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
) -> Option<Vec<Arc<dyn Definition>>> {
    let mut defs: Vec<Arc<dyn Definition>>;
    match &node.nodes.2 {
        FunctionBodyDeclaration::WithoutPort(x) => {
            let mut func = SubDec::new(url);
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
            defs = vec![Arc::new(func)];
        }
        FunctionBodyDeclaration::WithPort(x) => {
            let mut func = SubDec::new(url);
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
            defs = vec![Arc::new(func)];
            match &x.nodes.3.nodes.1 {
                Some(tfports) => {
                    skip_until_enter!(tree, event_iter, RefNode::TfPortList, &TfPortList);
                    let ports = tfport_list(tree, tfports, event_iter, url)?;
                    for port in ports {
                        defs.push(Arc::new(port));
                    }
                }
                None => (),
            }
        }
    }
    Some(defs)
}

pub fn task_dec(
    tree: &SyntaxTree,
    node: &TaskDeclaration,
    event_iter: &mut EventIter,
    url: &Url,
) -> Option<Vec<Arc<dyn Definition>>> {
    let mut defs: Vec<Arc<dyn Definition>>;
    match &node.nodes.2 {
        TaskBodyDeclaration::WithoutPort(x) => {
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
            task.kind = CompletionItemKind::Function;
            defs = vec![Arc::new(task)];
        }
        TaskBodyDeclaration::WithPort(x) => {
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
            task.kind = CompletionItemKind::Function;
            defs = vec![Arc::new(task)];
            match &x.nodes.2.nodes.1 {
                Some(tfports) => {
                    skip_until_enter!(tree, event_iter, RefNode::TfPortList, &TfPortList);
                    let ports = tfport_list(tree, tfports, event_iter, url)?;
                    for port in ports {
                        defs.push(Arc::new(port));
                    }
                }
                None => (),
            }
        }
    }
    Some(defs)
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
        modport.kind = CompletionItemKind::Interface;

        let mut mp_port_decs = vec![&modport_def.nodes.1.nodes.1.nodes.0];
        for mp_port_def in &modport_def.nodes.1.nodes.1.nodes.1 {
            mp_port_decs.push(&mp_port_def.1);
        }
        for mp_port_dec in mp_port_decs {
            match mp_port_dec {
                ModportPortsDeclaraton::Simple(x) => {
                    skip_until_enter!(
                        tree,
                        event_iter,
                        RefNode::ModportPortsDeclaratonSimple,
                        &ModportPortsDeclaratonSimple
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
                                port.kind = CompletionItemKind::Property;
                                port.type_str = prepend.clone();
                                modport.ports.push(Arc::new(port));
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
                                port.kind = CompletionItemKind::Property;
                                let mut append = String::new();
                                advance_until_leave!(
                                    append,
                                    tree,
                                    event_iter,
                                    RefNode::ModportSimplePortNamed
                                );
                                port.type_str = format!("{} {}", prepend, append);
                                modport.ports.push(Arc::new(port));
                            }
                        }
                    }
                }
                ModportPortsDeclaraton::Tf(x) => {
                    skip_until_enter!(
                        tree,
                        event_iter,
                        RefNode::ModportPortsDeclaratonTf,
                        &ModportPortsDeclaratonTf
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
                                    modport.ports.push(Arc::new(port));
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
                                    modport.ports.push(Arc::new(port));
                                }
                            },
                            ModportTfPort::TfIdentifier(y) => {
                                let mut port = SubDec::new(url);
                                let ident = get_ident(tree, RefNode::TfIdentifier(&y));
                                port.ident = ident.0;
                                port.byte_idx = ident.1;
                                port.type_str = prepend.clone();
                                modport.ports.push(Arc::new(port));
                            }
                        }
                    }
                }
                ModportPortsDeclaraton::Clocking(x) => {
                    skip_until_enter!(
                        tree,
                        event_iter,
                        RefNode::ModportPortsDeclaratonClocking,
                        &ModportPortsDeclaratonClocking
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
                    modport.ports.push(Arc::new(port));
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
