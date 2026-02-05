//! Build script that generates Python bindings from Rust function metadata.
//!
//! This runs during `cargo build` and generates:
//! - r4pm/bindings/__init__.py (main bindings module)
//! - r4pm/bindings/__init__.pyi (type stubs)
//! - r4pm/bindings/<module>/*.py (per-module wrappers)
//! - r4pm/bindings/<module>/*.pyi (per-module stubs)

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use process_mining::bindings::BindingMeta;

/// Base URL for Rust documentation
const DOCS_BASE_URL: &str = "https://rust4pm.aarkue.eu/docs";

/// Documentation version (for future versioning support)
const DOCS_VERSION: &str = env!("CARGO_PKG_VERSION");
const LANG: &str = "python";

fn main() {
    // Tell Cargo to rerun if these change
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let bindings_dir = Path::new(&manifest_dir).join("r4pm").join("bindings");

    // Clean and recreate bindings directory
    if bindings_dir.exists() {
        fs::remove_dir_all(&bindings_dir).expect("Failed to remove old bindings");
    }
    fs::create_dir_all(&bindings_dir).expect("Failed to create bindings directory");

    // Get function metadata from process_mining
    let functions = process_mining::bindings::list_functions_meta();

    // Group functions by module
    let mut module_groups: BTreeMap<String, Vec<BindingMeta>> = BTreeMap::new();
    for func in functions {
        let py_module = rust_module_to_python(&func.module);
        module_groups.entry(py_module).or_default().push(func);
    }

    // println!("cargo:warning=Generating bindings for {} modules", module_groups.len());

    // Generate module files
    for (module_path, funcs) in &module_groups {
        generate_module_files(&bindings_dir, module_path, funcs);
    }

    // Generate intermediate __init__.py files
    generate_submodule_inits(&bindings_dir, &module_groups);

    // Generate main bindings/__init__.py
    generate_bindings_init(&bindings_dir, &module_groups);
}

/// Convert Rust module path to Python module path
fn rust_module_to_python(rust_module: &str) -> String {
    rust_module
        .replace("process_mining::", "")
        .replace("::", ".")
}

/// Generate documentation URL for a Rust module
fn module_docs_url(rust_module: &str) -> String {
    let path = rust_module.replace("::", "/");
    format!("{}/{}?v={}&lang={}", DOCS_BASE_URL, path, DOCS_VERSION, LANG)
}

/// Generate documentation URL for a specific function
fn function_docs_url(func: &BindingMeta) -> String {
    let module_path = func.module.replace("::", "/");
    format!(
        "{}/{}?v={}&lang={}#{}",
        DOCS_BASE_URL, module_path, DOCS_VERSION, LANG, func.name,
    )
}

/// Generate a TypedDict class definition for a complex object schema
/// Uses string annotations (forward references) to avoid ordering issues
fn generate_typed_dict(
    name: &str,
    schema: &serde_json::Value,
    root_schema: &serde_json::Value,
    generated: &mut BTreeMap<String, String>,
) -> String {
    // Avoid regenerating - but still need to process to collect nested types
    if generated.contains_key(name) {
        return format!("\"{}\"", name); // Return as forward reference
    }

    // Mark as being generated to handle recursive types
    generated.insert(name.to_string(), String::new());

    let props = match schema.get("properties").and_then(|p| p.as_object()) {
        Some(p) => p,
        None => return "Dict[str, Any]".to_string(),
    };

    let required: BTreeSet<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    // Check if any field name is a Python reserved keyword
    let python_keywords = [
        "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
        "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
        "if", "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
        "try", "while", "with", "yield",
    ];
    let has_reserved_keyword = props.keys().any(|k| python_keywords.contains(&k.as_str()));

    let mut fields = Vec::new();
    let mut field_types = Vec::new();

    for (prop_name, prop_schema) in props {
        let prop_type = schema_to_python_type_inner(prop_schema, root_schema, generated);
        let is_required = required.contains(prop_name.as_str());

        // Get description for the field
        let desc = prop_schema
            .get("description")
            .and_then(|d| d.as_str())
            .map(|d| format!("  # {}", d.lines().next().unwrap_or("")))
            .unwrap_or_default();

        let full_type = if is_required {
            prop_type.clone()
        } else {
            format!("Optional[{}]", prop_type)
        };

        if has_reserved_keyword {
            // Use functional syntax - store for later
            field_types.push((prop_name.clone(), full_type, desc));
        } else {
            // Use class syntax
            fields.push(format!("    {}: {}{}", prop_name, full_type, desc));
        }
    }

    let class_def = if has_reserved_keyword {
        // Use functional TypedDict syntax for classes with reserved keywords
        // TypedDict("Name", {"from": Type, ...})
        let fields_str: Vec<String> = field_types
            .iter()
            .map(|(name, typ, desc)| format!("    \"{}\": {},{}", name, typ, desc))
            .collect();
        format!(
            "{} = TypedDict(\"{}\", {{\n{}\n}}, total=False)\n",
            name,
            name,
            fields_str.join("\n")
        )
    } else {
        format!(
            "class {}(TypedDict, total=False):\n{}\n",
            name,
            if fields.is_empty() {
                "    pass".to_string()
            } else {
                fields.join("\n")
            }
        )
    };

    generated.insert(name.to_string(), class_def);
    format!("\"{}\"", name) // Return as forward reference
}

/// Sanitize a name to be a valid Python identifier
fn sanitize_identifier(name: &str) -> String {
    name.replace('-', "_")
        .replace('+', "Plus")
        .replace(' ', "_")
        .replace("::", "_")
        .replace(".", "_")
}

/// Inner function for schema to Python type conversion (with TypedDict generation)
fn schema_to_python_type_inner(
    schema: &serde_json::Value,
    root_schema: &serde_json::Value,
    generated_types: &mut BTreeMap<String, String>,
) -> String {
    // Registry references are always string IDs
    if schema.get("x-registry-ref").is_some() {
        return "str".to_string();
    }

    // Handle $ref to definitions
    if let Some(ref_str) = schema.get("$ref").and_then(|r| r.as_str()) {
        if let Some(def_name) = ref_str.strip_prefix("#/$defs/") {
            if let Some(defs) = root_schema.get("$defs") {
                if let Some(def_schema) = defs.get(def_name) {
                    // Check if it has a title for naming
                    let type_name = def_schema
                        .get("title")
                        .and_then(|t| t.as_str())
                        .map(|t| sanitize_identifier(t))
                        .unwrap_or_else(|| sanitize_identifier(def_name));

                    // If it's an object with properties, generate TypedDict
                    if def_schema.get("properties").is_some() {
                        return generate_typed_dict(
                            &type_name,
                            def_schema,
                            root_schema,
                            generated_types,
                        );
                    }
                    return schema_to_python_type_inner(def_schema, root_schema, generated_types);
                }
            }
            return "Dict[str, Any]".to_string();
        }
    }

    // Handle oneOf (union types)
    if let Some(one_of) = schema.get("oneOf").and_then(|o| o.as_array()) {
        let types: Vec<String> = one_of
            .iter()
            .map(|s| schema_to_python_type_inner(s, root_schema, generated_types))
            .collect();
        if types.len() == 1 {
            return types[0].clone();
        }
        let mut unique: Vec<String> = types
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        unique.sort();
        return format!("Union[{}]", unique.join(", "));
    }

    // Handle anyOf
    if let Some(any_of) = schema.get("anyOf").and_then(|o| o.as_array()) {
        let types: Vec<String> = any_of
            .iter()
            .map(|s| schema_to_python_type_inner(s, root_schema, generated_types))
            .collect();
        if types.len() == 1 {
            return types[0].clone();
        }
        let mut unique: Vec<String> = types
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        unique.sort();
        return format!("Union[{}]", unique.join(", "));
    }

    // Handle array of types (e.g., ["string", "null"] for Optional)
    if let Some(types_arr) = schema.get("type").and_then(|t| t.as_array()) {
        let type_strs: Vec<&str> = types_arr.iter().filter_map(|t| t.as_str()).collect();
        if type_strs.len() == 2 && type_strs.contains(&"null") {
            let non_null = type_strs.iter().find(|&&t| t != "null").unwrap();
            let inner = schema_to_python_type_inner(
                &serde_json::json!({"type": non_null}),
                root_schema,
                generated_types,
            );
            return format!("Optional[{}]", inner);
        }
        let types: Vec<String> = type_strs
            .iter()
            .map(|t| match *t {
                "string" => "str".to_string(),
                "integer" => "int".to_string(),
                "number" => "float".to_string(),
                "boolean" => "bool".to_string(),
                "null" => "None".to_string(),
                "array" => "List[Any]".to_string(),
                "object" => "Dict[str, Any]".to_string(),
                _ => "Any".to_string(),
            })
            .collect();
        return format!("Union[{}]", types.join(", "));
    }

    match schema.get("type").and_then(|t| t.as_str()) {
        Some("string") => {
            if let Some(const_val) = schema.get("const").and_then(|c| c.as_str()) {
                return format!("Literal[\"{}\"]", const_val);
            }
            if let Some(enum_vals) = schema.get("enum").and_then(|e| e.as_array()) {
                let literals: Vec<String> = enum_vals
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| format!("\"{}\"", s))
                    .collect();
                if !literals.is_empty() {
                    return format!("Literal[{}]", literals.join(", "));
                }
            }
            "str".to_string()
        }
        Some("integer") => "int".to_string(),
        Some("number") => "float".to_string(),
        Some("boolean") => "bool".to_string(),
        Some("null") => "None".to_string(),
        Some("array") => {
            if let Some(prefix_items) = schema.get("prefixItems").and_then(|p| p.as_array()) {
                let item_types: Vec<String> = prefix_items
                    .iter()
                    .map(|item| schema_to_python_type_inner(item, root_schema, generated_types))
                    .collect();
                return format!("Tuple[{}]", item_types.join(", "));
            }
            let items_type = schema
                .get("items")
                .map(|i| schema_to_python_type_inner(i, root_schema, generated_types))
                .unwrap_or_else(|| "Any".to_string());
            format!("List[{}]", items_type)
        }
        Some("object") => {
            // Check for title - generate TypedDict
            if let Some(title) = schema.get("title").and_then(|t| t.as_str()) {
                if schema.get("properties").is_some() {
                    let type_name = sanitize_identifier(title);
                    return generate_typed_dict(&type_name, schema, root_schema, generated_types);
                }
            }
            // Check for additionalProperties
            if let Some(add_props) = schema.get("additionalProperties") {
                if add_props.is_object() {
                    let value_type =
                        schema_to_python_type_inner(add_props, root_schema, generated_types);
                    return format!("Dict[str, {}]", value_type);
                }
            }
            "Dict[str, Any]".to_string()
        }
        _ => "Any".to_string(),
    }
}

/// Sanitize function name for Python
fn sanitize_name(name: &str) -> String {
    name.replace('-', "_").replace('+', "plus")
}

/// Generate .py and .pyi files for a module
fn generate_module_files(bindings_dir: &Path, module_path: &str, functions: &[BindingMeta]) {
    let parts: Vec<&str> = module_path.split('.').collect();
    let import_depth = parts.len() + 1;

    // Create directory structure
    let module_dir = if parts.len() > 1 {
        bindings_dir.join(parts[..parts.len() - 1].join("/"))
    } else {
        bindings_dir.to_path_buf()
    };
    fs::create_dir_all(&module_dir).expect("Failed to create module directory");

    let file_name = parts.last().unwrap();

    // Get the Rust module path for docs URL
    let rust_module = format!("process_mining::{}", module_path.replace('.', "::"));
    let docs_url = module_docs_url(&rust_module);

    // Collect all TypedDict definitions needed for this module
    let mut generated_types: BTreeMap<String, String> = BTreeMap::new();
    let mut func_return_types: Vec<String> = Vec::new();
    let mut func_arg_types: Vec<Vec<String>> = Vec::new();

    for func in functions {
        // Generate return type with TypedDicts
        let ret_type =
            schema_to_python_type_inner(&func.return_type, &func.return_type, &mut generated_types);
        func_return_types.push(ret_type);

        // Generate arg types
        let mut arg_types = Vec::new();
        for (_, arg_schema) in &func.args {
            let arg_type =
                schema_to_python_type_inner(arg_schema, arg_schema, &mut generated_types);
            arg_types.push(arg_type);
        }
        func_arg_types.push(arg_types);
    }

    // Generate .py file
    let mut py_content = format!(
        r#"""""{module_path} - Python bindings for Rust4PM library.

Binding Documentation: {docs_url}

This module is auto-generated. For implementation details, see the documentation above or the Rust source.
"""
from __future__ import annotations
from typing import Any, Dict, List, Literal, Optional, Tuple, TypedDict, Union

"#,
        module_path = module_path,
        docs_url = docs_url,
    );

    // Add TypedDict definitions
    for (_, type_def) in &generated_types {
        py_content.push_str(type_def);
        py_content.push_str("\n");
    }

    let mut func_names = Vec::new();
    for (i, func) in functions.iter().enumerate() {
        let py_name = sanitize_name(&func.name);
        func_names.push(py_name.clone());
        py_content.push_str(&generate_function_wrapper_with_types(
            func,
            import_depth,
            &func_return_types[i],
            &func_arg_types[i],
        ));
        py_content.push_str("\n\n");
    }

    py_content.push_str("__all__ = [\n");
    for name in &func_names {
        py_content.push_str(&format!("    \"{}\",\n", name));
    }
    py_content.push_str("]\n");

    fs::write(module_dir.join(format!("{}.py", file_name)), py_content)
        .expect("Failed to write .py file");

    // Generate .pyi file
    let mut pyi_content = format!(
        r#""""{module_path} - Type stubs for process_mining Rust bindings.

Rust Documentation: {docs_url}

This module is auto-generated. For implementation details, see the Rust source.
"""
from __future__ import annotations
from typing import Any, Dict, List, Literal, Optional, Tuple, TypedDict, Union

"#,
        module_path = module_path,
        docs_url = docs_url,
    );

    // Add TypedDict definitions
    for (_, type_def) in &generated_types {
        pyi_content.push_str(type_def);
        pyi_content.push_str("\n");
    }

    for (i, func) in functions.iter().enumerate() {
        pyi_content.push_str(&generate_function_stub_with_types(
            func,
            &func_return_types[i],
            &func_arg_types[i],
        ));
        pyi_content.push_str("\n\n");
    }

    pyi_content.push_str("__all__: List[str]\n");

    fs::write(module_dir.join(format!("{}.pyi", file_name)), pyi_content)
        .expect("Failed to write .pyi file");
}

/// Generate a Python wrapper function with pre-computed types
fn generate_function_wrapper_with_types(
    func: &BindingMeta,
    import_depth: usize,
    return_type: &str,
    arg_types: &[String],
) -> String {
    let py_name = sanitize_name(&func.name);
    let required_args: BTreeSet<&str> = func.required_args.iter().map(|s| s.as_str()).collect();
    let docs_url = function_docs_url(func);

    // Build parameters
    let mut params = Vec::new();
    let mut doc_args = Vec::new();
    let mut dict_lines = vec!["    args_dict = {}".to_string()];

    for (i, (arg_name, arg_schema)) in func.args.iter().enumerate() {
        let arg_type = &arg_types[i];
        let is_required = required_args.contains(arg_name.as_str());

        if is_required {
            params.push(format!("{}: {}", arg_name, arg_type));
            dict_lines.push(format!("    args_dict[\"{}\"] = {}", arg_name, arg_name));
        } else {
            params.push(format!("{}: Optional[{}] = None", arg_name, arg_type));
            dict_lines.push(format!("    if {} is not None:", arg_name));
            dict_lines.push(format!(
                "        args_dict[\"{}\"] = {}",
                arg_name, arg_name
            ));
        }

        // Doc line
        if arg_schema.get("x-registry-ref").is_some() {
            let title = arg_schema
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("item");
            doc_args.push(format!("        {}: Registry ID for {}", arg_name, title));
        } else if let Some(desc) = arg_schema.get("description").and_then(|d| d.as_str()) {
            doc_args.push(format!(
                "        {}: {}",
                arg_name,
                desc.lines().next().unwrap_or("")
            ));
        } else {
            doc_args.push(format!("        {}: {}", arg_name, arg_type));
        }
    }

    // Build docstring
    let mut doc_lines: Vec<String> = func.docs.iter().map(|s| format!("    {}", s)).collect();

    // Add docs link
    doc_lines.push("".to_string());
    doc_lines.push(format!("    See: {}", docs_url));

    if !doc_args.is_empty() {
        doc_lines.push("".to_string());
        doc_lines.push("    Args:".to_string());
        doc_lines.extend(doc_args);
    }
    doc_lines.push("".to_string());
    doc_lines.push("    Returns:".to_string());
    if func.return_type.get("x-registry-ref").is_some() {
        let title = func
            .return_type
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("result");
        doc_lines.push(format!("        Registry ID for {}", title));
    } else {
        doc_lines.push(format!("        {}", return_type));
    }

    let import_prefix = ".".repeat(import_depth);

    format!(
        r#"def {py_name}({params}) -> {return_type}:
    """
{docstring}
    """
    import json
    from {import_prefix}r4pm import call_binding
{dict_lines}
    result = call_binding("{func_id}", json.dumps(args_dict))
    return json.loads(result)"#,
        py_name = py_name,
        params = params.join(", "),
        return_type = return_type,
        docstring = doc_lines.join("\n"),
        import_prefix = import_prefix,
        dict_lines = dict_lines.join("\n"),
        func_id = func.id,
    )
}

/// Generate a stub function for .pyi with pre-computed types
fn generate_function_stub_with_types(
    func: &BindingMeta,
    return_type: &str,
    arg_types: &[String],
) -> String {
    let py_name = sanitize_name(&func.name);
    let required_args: BTreeSet<&str> = func.required_args.iter().map(|s| s.as_str()).collect();
    let docs_url = function_docs_url(func);

    let mut params = Vec::new();
    let mut doc_args = Vec::new();

    for (i, (arg_name, arg_schema)) in func.args.iter().enumerate() {
        let arg_type = &arg_types[i];
        let is_required = required_args.contains(arg_name.as_str());

        if is_required {
            params.push(format!("{}: {}", arg_name, arg_type));
        } else {
            params.push(format!("{}: Optional[{}] = None", arg_name, arg_type));
        }

        if arg_schema.get("x-registry-ref").is_some() {
            let title = arg_schema
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or("item");
            doc_args.push(format!("        {}: Registry ID for {}", arg_name, title));
        } else if let Some(desc) = arg_schema.get("description").and_then(|d| d.as_str()) {
            doc_args.push(format!(
                "        {}: {}",
                arg_name,
                desc.lines().next().unwrap_or("")
            ));
        } else {
            doc_args.push(format!("        {}: {}", arg_name, arg_type));
        }
    }

    // Docstring
    let mut doc_lines: Vec<String> = func.docs.iter().map(|s| format!("    {}", s)).collect();

    // Add docs link
    doc_lines.push("".to_string());
    doc_lines.push(format!("    See: {}", docs_url));

    if !doc_args.is_empty() {
        doc_lines.push("".to_string());
        doc_lines.push("    Args:".to_string());
        doc_lines.extend(doc_args);
    }
    doc_lines.push("".to_string());
    doc_lines.push("    Returns:".to_string());
    doc_lines.push(format!("        {}", return_type));

    format!(
        r#"def {py_name}({params}) -> {return_type}:
    """
{docstring}
    """
    ..."#,
        py_name = py_name,
        params = params.join(", "),
        return_type = return_type,
        docstring = doc_lines.join("\n"),
    )
}

/// Generate __init__.py files for intermediate directories
fn generate_submodule_inits(
    bindings_dir: &Path,
    module_groups: &BTreeMap<String, Vec<BindingMeta>>,
) {
    // Collect directory structure
    let mut dir_contents: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();

    for module_path in module_groups.keys() {
        let parts: Vec<&str> = module_path.split('.').collect();

        // Track file at leaf level
        if parts.len() == 1 {
            dir_contents
                .entry(String::new())
                .or_default()
                .1
                .insert(parts[0].to_string());
        } else {
            let parent = parts[..parts.len() - 1].join("/");
            dir_contents
                .entry(parent)
                .or_default()
                .1
                .insert(parts.last().unwrap().to_string());
        }

        // Track intermediate directories
        for i in 1..parts.len() {
            let parent = if i > 1 {
                parts[..i - 1].join("/")
            } else {
                String::new()
            };
            dir_contents
                .entry(parent)
                .or_default()
                .0
                .insert(parts[i - 1].to_string());
        }
    }

    // Generate __init__.py for each non-root directory
    for (dir_path, (submodules, files)) in &dir_contents {
        if dir_path.is_empty() {
            continue; // Root handled separately
        }

        let target_dir = bindings_dir.join(dir_path);
        fs::create_dir_all(&target_dir).expect("Failed to create submodule directory");

        let mut init_content = "\"\"\"Auto-generated module.\"\"\"\n\n".to_string();
        let mut all_items = Vec::new();

        for submod in submodules {
            init_content.push_str(&format!("from . import {}\n", submod));
            all_items.push(submod.clone());
        }

        for file_mod in files {
            init_content.push_str(&format!("from .{} import *\n", file_mod));
        }

        init_content.push_str(&format!("\n__all__ = {:?}\n", all_items));

        fs::write(target_dir.join("__init__.py"), &init_content)
            .expect("Failed to write submodule __init__.py");

        // Stub file - need explicit __all__ for proper re-export recognition
        let mut stub_content = "\"\"\"Type stubs.\"\"\"\nfrom __future__ import annotations\nfrom typing import List\n\n".to_string();
        for submod in submodules {
            stub_content.push_str(&format!("from . import {} as {}\n", submod, submod));
        }
        for file_mod in files {
            stub_content.push_str(&format!("from .{} import *\n", file_mod));
        }

        // Generate explicit __all__ list
        stub_content.push_str("\n__all__ = [\n");
        for submod in submodules {
            stub_content.push_str(&format!("    \"{}\",\n", submod));
        }
        // Note: items from `from .x import *` are implicitly included via the * import
        stub_content.push_str("]\n");

        fs::write(target_dir.join("__init__.pyi"), stub_content)
            .expect("Failed to write submodule __init__.pyi");
    }
}

/// Generate the main bindings/__init__.py
fn generate_bindings_init(bindings_dir: &Path, module_groups: &BTreeMap<String, Vec<BindingMeta>>) {
    let mut top_modules: BTreeSet<String> = BTreeSet::new();
    let mut all_funcs: Vec<(String, String, &BindingMeta)> = Vec::new();

    for (module_path, functions) in module_groups {
        let parts: Vec<&str> = module_path.split('.').collect();
        top_modules.insert(parts[0].to_string());

        for func in functions {
            let py_name = sanitize_name(&func.name);
            all_funcs.push((py_name, module_path.clone(), func));
        }
    }

    // Sort for deterministic output
    all_funcs.sort_by(|a, b| a.0.cmp(&b.0));

    let bindings_docs_url = format!("{}/process_mining/bindings?{}&lang={}", DOCS_BASE_URL, DOCS_VERSION, LANG);

    let mut content = format!(
        r#""""r4pm.bindings - Python bindings for the process_mining Rust library.

This module provides Python wrappers for Rust process mining algorithms.
All functions are auto-generated from Rust function metadata.

Rust Documentation: {docs_url}

Usage:
    from r4pm.bindings import discover_dfg
    # or access via submodules:
    from r4pm.bindings.discovery.case_centric import discover_dfg
"""
from __future__ import annotations
from typing import Any, Dict, List, Literal, Optional, Tuple, TypedDict, Union

"#,
        docs_url = bindings_docs_url,
    );

    // Import submodules
    for module in &top_modules {
        content.push_str(&format!("from . import {}\n", module));
    }

    content.push_str("\n# Re-export all functions at top level\n");
    for (func_name, module_path, _) in &all_funcs {
        let parts: Vec<&str> = module_path.split('.').collect();
        content.push_str(&format!("from .{} import {}\n", parts.join("."), func_name));
    }

    content.push_str("\n__all__ = [\n");
    for module in &top_modules {
        content.push_str(&format!("    \"{}\",\n", module));
    }
    for (func_name, _, _) in &all_funcs {
        content.push_str(&format!("    \"{}\",\n", func_name));
    }
    content.push_str("]\n");

    fs::write(bindings_dir.join("__init__.py"), &content)
        .expect("Failed to write bindings __init__.py");

    // Generate stub file with full function declarations for better IDE support
    let mut stub_content = format!(
        r#""""r4pm.bindings - Type stubs for process_mining Rust bindings.

Rust Documentation: {docs_url}

This module is auto-generated. For implementation details, see the Rust source.
"""
from __future__ import annotations
from typing import Any, Dict, List, Literal, Optional, Tuple, TypedDict, Union

"#,
        docs_url = bindings_docs_url,
    );

    for module in &top_modules {
        stub_content.push_str(&format!("from . import {} as {}\n", module, module));
    }
    stub_content.push_str("\n");

    // Generate TypedDicts and full function stubs at top level for better IDE support
    let mut generated_types: BTreeMap<String, String> = BTreeMap::new();
    let mut func_stubs: Vec<String> = Vec::new();

    for (_, _, func) in &all_funcs {
        // Generate types for this function
        let ret_type =
            schema_to_python_type_inner(&func.return_type, &func.return_type, &mut generated_types);
        let mut arg_types = Vec::new();
        for (_, arg_schema) in &func.args {
            let arg_type =
                schema_to_python_type_inner(arg_schema, arg_schema, &mut generated_types);
            arg_types.push(arg_type);
        }

        // Generate the stub
        func_stubs.push(generate_function_stub_with_types(
            func, &ret_type, &arg_types,
        ));
    }

    // Add TypedDict definitions
    for (_, type_def) in &generated_types {
        if !type_def.is_empty() {
            stub_content.push_str(type_def);
            stub_content.push_str("\n");
        }
    }

    // Add function stubs
    for stub in func_stubs {
        stub_content.push_str(&stub);
        stub_content.push_str("\n\n");
    }

    // Generate explicit __all__ list for proper re-export recognition
    stub_content.push_str("__all__ = [\n");
    for module in &top_modules {
        stub_content.push_str(&format!("    \"{}\",\n", module));
    }
    for (func_name, _, _) in &all_funcs {
        stub_content.push_str(&format!("    \"{}\",\n", func_name));
    }
    stub_content.push_str("]\n");

    fs::write(bindings_dir.join("__init__.pyi"), stub_content)
        .expect("Failed to write bindings __init__.pyi");
}
