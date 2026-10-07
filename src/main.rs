use quote::ToTokens;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as IoWrite;
use syn::{Attribute, Expr, ExprLit, ImplItem, Item, Lit, Meta, Type};
use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().collect();

    if args.get(1).map(|s| s.as_str()) == Some("doc-extract") {
        args.remove(1);
    }

    let src_dir = args.get(1).map(|s| s.as_str()).unwrap_or("src");
    let output_file = args.get(2).map(|s| s.as_str()).unwrap_or("API_SUMMARY.md");

    let mut out = fs::File::create(output_file)?;
    writeln!(out, "# API Summary\n")?;

    for entry in WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
    {
        let path = entry.path();
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(parsed) = syn::parse_file(&content) {
                let mut file_out = String::new();
                extract_file_items(&parsed.items, &mut file_out);
                
                // Only print the file header if we actually extracted structs/fns
                if !file_out.trim().is_empty() {
                    writeln!(out, "## `{}`\n", path.display())?;
                    write!(out, "{}", file_out)?;
                }
            }
        }
    }

    println!("Succinct API summary generated at `{}`!", output_file);
    Ok(())
}

fn extract_file_items(items: &[Item], out: &mut String) {
    for item in items {
        match item {
            Item::Struct(s) => {
                let docs = extract_docs_inline(&s.attrs);
                writeln!(out, "- **`struct {}`**{}", s.ident, docs).unwrap();
            }
            Item::Enum(e) => {
                let docs = extract_docs_inline(&e.attrs);
                let vars: Vec<_> = e.variants.iter().map(|v| v.ident.to_string()).collect();
                writeln!(out, "- **`enum {}`** `[{}]`{}", e.ident, vars.join(", "), docs).unwrap();
            }
            Item::Trait(t) => {
                let docs = extract_docs_inline(&t.attrs);
                writeln!(out, "- **`trait {}`**{}", t.ident, docs).unwrap();
                for ti in &t.items {
                    if let syn::TraitItem::Fn(m) = ti {
                        let sig = m.sig.to_token_stream().to_string();
                        writeln!(out, "  - `{}`{}", sig, extract_docs_inline(&m.attrs)).unwrap();
                    }
                }
            }
            Item::Impl(imp) => {
                let self_ty = match clean_type_name(&imp.self_ty) {
                    Some(name) => name,
                    None => continue,
                };
                let header = match imp.trait_.as_ref() {
                    Some((path, _)) => format!("impl {} for {}", path.to_token_stream(), self_ty),
                    None => format!("impl {}", self_ty),
                };

                let mut fns = Vec::new();
                for item in &imp.items {
                    if let ImplItem::Fn(m) = item {
                        fns.push((m.sig.to_token_stream().to_string(), extract_docs_inline(&m.attrs)));
                    }
                }
                if !fns.is_empty() {
                    writeln!(out, "- **`{}`**", header).unwrap();
                    for (sig, docs) in fns {
                        writeln!(out, "  - `{}`{}", sig, docs).unwrap();
                    }
                }
            }
            Item::Fn(f) => {
                let sig = f.sig.to_token_stream().to_string();
                writeln!(out, "- `{}`{}", sig, extract_docs_inline(&f.attrs)).unwrap();
            }
            Item::Mod(m) => {
                if let Some((_, inline_items)) = &m.content {
                    extract_file_items(inline_items, out);
                }
            }
            _ => {}
        }
    }
}

/// Extracts triple-slash `///` docs and formats them as a single inline string.
fn extract_docs_inline(attrs: &[Attribute]) -> String {
    let mut doc_lines = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value {
                    let text = s.value().trim().to_string();
                    if !text.is_empty() {
                        doc_lines.push(text);
                    }
                }
            }
        }
    }

    if doc_lines.is_empty() {
        String::new()
    } else {
        // Join multiline comments into a single line with spaces
        format!(": *{}*", doc_lines.join(" "))
    }
}

fn clean_type_name(ty: &Type) -> Option<String> {
    Some(ty.to_token_stream().to_string())
}