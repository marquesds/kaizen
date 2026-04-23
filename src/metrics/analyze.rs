// SPDX-License-Identifier: AGPL-3.0-or-later
//! Multi-language file analyzers. Tree-sitter when supported.

use crate::metrics::types::{RepoAnalysis, SymbolFact};
use anyhow::Result;
use std::path::Path;
use tree_sitter::{Language, Node, Parser};

pub trait CodeAnalyzer {
    fn analyze(&self, rel_path: &str, source: &str) -> Result<RepoAnalysis>;
}

pub fn analyzer_for(path: &Path) -> Box<dyn CodeAnalyzer> {
    language_spec(path)
        .map(|spec| Box::new(TreeSitterAnalyzer { spec }) as Box<dyn CodeAnalyzer>)
        .unwrap_or_else(|| Box::new(GenericAnalyzer))
}

pub struct GenericAnalyzer;

impl CodeAnalyzer for GenericAnalyzer {
    fn analyze(&self, rel_path: &str, source: &str) -> Result<RepoAnalysis> {
        let lines: Vec<&str> = source.lines().collect();
        Ok(RepoAnalysis {
            path: rel_path.into(),
            language: language_name(Path::new(rel_path)).into(),
            bytes: source.len() as u64,
            loc: lines.len() as u32,
            sloc: lines.iter().filter(|line| !line.trim().is_empty()).count() as u32,
            complexity_total: 0,
            max_fn_complexity: 0,
            imports: vec![],
            symbols: vec![],
        })
    }
}

struct TreeSitterAnalyzer {
    spec: &'static LanguageSpec,
}

impl CodeAnalyzer for TreeSitterAnalyzer {
    fn analyze(&self, rel_path: &str, source: &str) -> Result<RepoAnalysis> {
        let mut parser = Parser::new();
        parser.set_language(&(self.spec.language)())?;
        let tree = parser.parse(source, None).expect("tree-sitter parse");
        let root = tree.root_node();
        let bytes = source.as_bytes();
        let mut symbols = vec![];
        collect_symbols(root, bytes, self.spec, &mut symbols);
        let imports = collect_kind_text(root, bytes, self.spec.import_kinds)
            .into_iter()
            .flat_map(|raw| extract_import_targets(&raw))
            .collect::<Vec<_>>();
        let symbol_ranges = symbols
            .iter()
            .map(|s| (s.start_byte, s.end_byte))
            .collect::<Vec<_>>();
        let top_level = count_top_level_complexity(root, bytes, self.spec, &symbol_ranges);
        let sum_symbols = symbols.iter().map(|s| s.complexity).sum::<u32>();
        Ok(RepoAnalysis {
            path: rel_path.into(),
            language: self.spec.name.into(),
            bytes: source.len() as u64,
            loc: source.lines().count() as u32,
            sloc: source
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count() as u32,
            complexity_total: sum_symbols + top_level,
            max_fn_complexity: symbols.iter().map(|s| s.complexity).max().unwrap_or(0),
            imports,
            symbols,
        })
    }
}

struct LanguageSpec {
    name: &'static str,
    language: fn() -> Language,
    symbol_kinds: &'static [&'static str],
    import_kinds: &'static [&'static str],
    call_kinds: &'static [&'static str],
    branch_kinds: &'static [&'static str],
}

fn language_spec(path: &Path) -> Option<&'static LanguageSpec> {
    static SPECS: &[LanguageSpec] = &[
        LanguageSpec {
            name: "rust",
            language: || tree_sitter_rust::LANGUAGE.into(),
            symbol_kinds: &[
                "function_item",
                "struct_item",
                "enum_item",
                "impl_item",
                "trait_item",
            ],
            import_kinds: &["use_declaration"],
            call_kinds: &["call_expression", "macro_invocation"],
            branch_kinds: &[
                "if_expression",
                "for_expression",
                "loop_expression",
                "while_expression",
                "match_expression",
            ],
        },
        LanguageSpec {
            name: "typescript",
            language: || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            symbol_kinds: &[
                "function_declaration",
                "method_definition",
                "class_declaration",
                "interface_declaration",
            ],
            import_kinds: &["import_statement"],
            call_kinds: &["call_expression"],
            branch_kinds: &[
                "if_statement",
                "for_statement",
                "for_in_statement",
                "while_statement",
                "switch_statement",
                "ternary_expression",
                "logical_expression",
            ],
        },
        LanguageSpec {
            name: "javascript",
            language: || tree_sitter_javascript::LANGUAGE.into(),
            symbol_kinds: &[
                "function_declaration",
                "method_definition",
                "class_declaration",
            ],
            import_kinds: &["import_statement"],
            call_kinds: &["call_expression"],
            branch_kinds: &[
                "if_statement",
                "for_statement",
                "for_in_statement",
                "while_statement",
                "switch_statement",
                "ternary_expression",
                "logical_expression",
            ],
        },
        LanguageSpec {
            name: "python",
            language: || tree_sitter_python::LANGUAGE.into(),
            symbol_kinds: &["function_definition", "class_definition"],
            import_kinds: &["import_statement", "import_from_statement"],
            call_kinds: &["call"],
            branch_kinds: &[
                "if_statement",
                "for_statement",
                "while_statement",
                "conditional_expression",
            ],
        },
        LanguageSpec {
            name: "go",
            language: || tree_sitter_go::LANGUAGE.into(),
            symbol_kinds: &[
                "function_declaration",
                "method_declaration",
                "type_declaration",
            ],
            import_kinds: &["import_declaration"],
            call_kinds: &["call_expression"],
            branch_kinds: &[
                "if_statement",
                "for_statement",
                "expression_switch_statement",
                "type_switch_statement",
                "select_statement",
            ],
        },
        LanguageSpec {
            name: "java",
            language: || tree_sitter_java::LANGUAGE.into(),
            symbol_kinds: &[
                "method_declaration",
                "class_declaration",
                "interface_declaration",
                "enum_declaration",
            ],
            import_kinds: &["import_declaration"],
            call_kinds: &["method_invocation"],
            branch_kinds: &[
                "if_statement",
                "for_statement",
                "enhanced_for_statement",
                "while_statement",
                "switch_expression",
                "switch_block",
            ],
        },
    ];
    let name = language_name(path);
    SPECS.iter().find(|spec| spec.name == name)
}

fn language_name(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        _ => "generic",
    }
}

fn collect_symbols(node: Node<'_>, source: &[u8], spec: &LanguageSpec, out: &mut Vec<SymbolFact>) {
    if spec.symbol_kinds.iter().any(|kind| *kind == node.kind()) {
        let calls = collect_kind_text(node, source, spec.call_kinds)
            .into_iter()
            .filter_map(|raw| call_name(&raw))
            .collect::<Vec<_>>();
        out.push(SymbolFact {
            path: String::new(),
            name: symbol_name(node, source),
            kind: node.kind().into(),
            complexity: 1 + count_complexity(node, spec),
            calls,
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
        });
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols(child, source, spec, out);
    }
}

fn symbol_name(node: Node<'_>, source: &[u8]) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if (child.kind().contains("identifier") || child.kind().ends_with("name"))
            && let Ok(text) = child.utf8_text(source)
        {
            return text.trim().to_string();
        }
    }
    node.kind().into()
}

fn collect_kind_text(node: Node<'_>, source: &[u8], kinds: &[&str]) -> Vec<String> {
    let mut out = vec![];
    collect_kind_text_inner(node, source, kinds, &mut out);
    out
}

fn collect_kind_text_inner(node: Node<'_>, source: &[u8], kinds: &[&str], out: &mut Vec<String>) {
    if kinds.iter().any(|kind| *kind == node.kind())
        && let Ok(text) = node.utf8_text(source)
    {
        out.push(text.to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_kind_text_inner(child, source, kinds, out);
    }
}

fn count_complexity(node: Node<'_>, spec: &LanguageSpec) -> u32 {
    let mut count = if spec.branch_kinds.iter().any(|kind| *kind == node.kind()) {
        1
    } else {
        0
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_complexity(child, spec);
    }
    count
}

fn count_top_level_complexity(
    root: Node<'_>,
    _source: &[u8],
    spec: &LanguageSpec,
    symbol_ranges: &[(usize, usize)],
) -> u32 {
    collect_kind_nodes(root, spec.branch_kinds)
        .into_iter()
        .filter(|node| !inside_symbol(node.start_byte(), symbol_ranges))
        .count() as u32
}

fn collect_kind_nodes<'a>(node: Node<'a>, kinds: &[&str]) -> Vec<Node<'a>> {
    let mut out = vec![];
    collect_kind_nodes_inner(node, kinds, &mut out);
    out
}

fn collect_kind_nodes_inner<'a>(node: Node<'a>, kinds: &[&str], out: &mut Vec<Node<'a>>) {
    if kinds.iter().any(|kind| *kind == node.kind()) {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_kind_nodes_inner(child, kinds, out);
    }
}

fn inside_symbol(byte: usize, ranges: &[(usize, usize)]) -> bool {
    ranges
        .iter()
        .any(|(start, end)| *start <= byte && byte < *end)
}

fn extract_import_targets(raw: &str) -> Vec<String> {
    let mut out = vec![];
    for quote in ['"', '\''] {
        if let Some(rest) = raw.split(quote).nth(1) {
            out.push(rest.to_string());
            return out;
        }
    }
    let cleaned = raw
        .replace("use ", "")
        .replace("import ", "")
        .replace("from ", "")
        .replace(';', "");
    let target = cleaned
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_matches('{')
        .trim_matches('}')
        .trim();
    if !target.is_empty() {
        out.push(target.into());
    }
    out
}

fn call_name(raw: &str) -> Option<String> {
    let head = raw.split('(').next()?.trim();
    let name = head.rsplit(['.', ':']).next()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.into())
}
