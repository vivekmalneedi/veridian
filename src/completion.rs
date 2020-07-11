use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::str;
use sv_parser::{parse_sv, unwrap_node, Locate, RefNode};
use trie_rs::TrieBuilder;

fn test() {
    println!("{}", env::current_dir().unwrap().display());

    // The path of SystemVerilog source file
    let path = PathBuf::from("tests/lab6/src/fp_add.sv");
    // The list of defined macros
    let defines = HashMap::new();
    // The list of include paths
    let includes: Vec<PathBuf> = Vec::new();

    // Parse
    let result = parse_sv(&path, &defines, &includes, false);
    // let mut identifiers = HashSet::new();
    let mut builder = TrieBuilder::new();

    if let Ok((syntax_tree, _)) = result {
        // &SyntaxTree is iterable
        for node in &syntax_tree {
            // The type of each node is RefNode
            // println!("{}", node);

            match node {
                RefNode::Identifier(x) => {
                    let id = unwrap_node!(x, Identifier).unwrap();
                    let id = get_identifier(id).unwrap();
                    let id_str = syntax_tree.get_str(&id).unwrap();
                    // identifiers.insert(id_str.to_owned());
                    builder.push(id_str.to_owned());
                }

                _ => (),
            }
        }
    } else {
        if let Err(error) = result {
            println!("Parse failed: {}", error);
        }
    }

    let trie = builder.build();

    let results: Vec<Vec<u8>> = trie.predictive_search("exp");
    let results_in_str: Vec<&str> = results
        .iter()
        .map(|u8s| str::from_utf8(u8s).unwrap())
        .collect();
    for id in results_in_str.iter() {
        println!("{}", id);
    }
}

fn get_identifier(node: RefNode) -> Option<Locate> {
    // unwrap_node! can take multiple types
    match unwrap_node!(node, SimpleIdentifier, EscapedIdentifier) {
        Some(RefNode::SimpleIdentifier(x)) => {
            return Some(x.nodes.0);
        }
        Some(RefNode::EscapedIdentifier(x)) => {
            return Some(x.nodes.0);
        }
        _ => None,
    }
}
