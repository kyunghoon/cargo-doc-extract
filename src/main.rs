use quote::ToTokens;
use std::fs;
use std::io::Write;
use syn::{
    Attribute, Expr, ExprLit, ImplItem, Item, Lit, Meta, Type,
};
use walkdir::WalkDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().collect();

    // If invoked via `cargo doc-extract`, drop the subcommand name
    if args.get(1).map(|s| s.as_str()) == Some("doc-extract") {
        args.remove(1);
    }

    let src_dir = args.get(1).map(|s| s.as_str()).unwrap_or("src");
    let output_file = args.get(2).map(|s| s.as_str()).unwrap_or("API_SUMMARY.md");

    let mut out = fs::File::create(output_file)?;

    writeln!(out, "# Project API Summary\n")?;
    writeln!(
        out,
        "> Automatically extracted structs, enums, traits, methods, and doc comments.\n"
    )?;

    for entry in WalkDir::new(src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rs"))
    {
        let path = entry.path();
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Ok(parsed) = syn::parse_file(&content) {
            writeln!(out, "## File: `{}`\n", path.display())?;
            extract_file_items(&parsed.items, &mut out)?;
        }
    }

    println!("Successfully generated API summary at `{}`!", output_file);
    Ok(())
}

fn extract_file_items(items: &[Item], out: &mut fs::File) -> std::io::Result<()> {
    for item in items {
        match item {
            Item::Struct(s) => {
                writeln!(out, "### `struct {}`\n", s.ident)?;
                write_doc_comments(&s.attrs, out)?;
                writeln!(
                    out,
                    "```rust\npub struct {} {{ /* fields omitted */ }}\n```\n",
                    s.ident
                )?;
            }
            Item::Enum(e) => {
                writeln!(out, "### `enum {}`\n", e.ident)?;
                write_doc_comments(&e.attrs, out)?;
                
                let variants: Vec<String> = e
                    .variants
                    .iter()
                    .map(|v| v.ident.to_string())
                    .collect();
                writeln!(
                    out,
                    "**Variants:** `{}`\n",
                    variants.join("`, `")
                )?;
            }
            Item::Trait(t) => {
                writeln!(out, "### `trait {}`\n", t.ident)?;
                write_doc_comments(&t.attrs, out)?;

                let mut trait_items = Vec::new();
                for ti in &t.items {
                    if let syn::TraitItem::Fn(m) = ti {
                        let sig = m.sig.to_token_stream().to_string();
                        let docs = extract_docs(&m.attrs);
                        trait_items.push((sig, docs));
                    }
                }

                if !trait_items.is_empty() {
                    writeln!(out, "**Trait Methods:**\n")?;
                    for (sig, docs) in trait_items {
                        writeln!(out, "- `{}`", sig)?;
                        if !docs.is_empty() {
                            for line in docs.lines() {
                                writeln!(out, "  > {}", line)?;
                            }
                        }
                    }
                    writeln!(out)?;
                }
            }
            Item::Impl(imp) => {
                let self_ty = match clean_type_name(&imp.self_ty) {
                    Some(name) => name,
                    None => continue,
                };

                let trait_name = imp
                    .trait_
                    .as_ref()
                    .map(|(path, _)| path.to_token_stream().to_string());

                let header = match trait_name {
                    Some(tr) => format!("`impl {} for {}`", tr, self_ty),
                    None => format!("`impl {}`", self_ty),
                };

                let mut fn_entries = Vec::new();

                for item in &imp.items {
                    if let ImplItem::Fn(m) = item {
                        let sig = m.sig.to_token_stream().to_string();
                        let docs = extract_docs(&m.attrs);
                        fn_entries.push((sig, docs));
                    }
                }

                if !fn_entries.is_empty() {
                    writeln!(out, "#### Impl Methods: {}\n", header)?;
                    for (sig, docs) in fn_entries {
                        writeln!(out, "- `{}`", sig)?;
                        if !docs.is_empty() {
                            for line in docs.lines() {
                                writeln!(out, "  > {}", line)?;
                            }
                        }
                    }
                    writeln!(out)?;
                }
            }
            Item::Fn(f) => {
                let sig = f.sig.to_token_stream().to_string();
                writeln!(out, "### `fn {}`\n", f.sig.ident)?;
                write_doc_comments(&f.attrs, out)?;
                writeln!(out, "```rust\n{};\n```\n", sig)?;
            }
            Item::Mod(m) => {
                if let Some((_, inline_items)) = &m.content {
                    extract_file_items(inline_items, out)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Helper to write extracted doc comments as Markdown blockquotes
fn write_doc_comments(attrs: &[Attribute], out: &mut fs::File) -> std::io::Result<()> {
    let docs = extract_docs(attrs);
    if !docs.is_empty() {
        for line in docs.lines() {
            writeln!(out, "> {}", line)?;
        }
        writeln!(out)?;
    }
    Ok(())
}

/// Extracts triple-slash `///` and `#[doc = "..."]` attributes into string lines
fn extract_docs(attrs: &[Attribute]) -> String {
    let mut doc_lines = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) = &nv.value
                {
                    doc_lines.push(s.value().trim().to_string());
                }
            }
        }
    }

    doc_lines.join("\n")
}

/// Helper to format type names neatly without full paths
fn clean_type_name(ty: &Type) -> Option<String> {
    Some(ty.to_token_stream().to_string())
}