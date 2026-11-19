//! Build script that generates Python bindings from Rust function metadata.
//!
//! This runs during `cargo build` and generates:
//! - r4pm/bindings/__init__.py (main bindings module) and its stub
//! - r4pm/bindings/_types.py (every TypedDict, emitted once) and its stub
//! - r4pm/bindings/<module>/*.py (per-module wrappers) and their stubs

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use process_mining::bindings::BindingMeta;
use serde_json::Value;

/// Base URL for Rust documentation
const DOCS_BASE_URL: &str = "https://rust4pm.aarkue.eu/docs";

/// Documentation version (for future versioning support)
const DOCS_VERSION: &str = env!("CARGO_PKG_VERSION");
const LANG: &str = "python";

const TYPES_MODULE: &str = "r4pm.bindings._types";

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class",
    "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if",
    "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try",
    "while", "with", "yield",
];

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    generate_enabled_features();

    let funcs = process_mining::bindings::list_functions_meta();

    println!("cargo:rerun-if-env-changed=R4PM_EXPORT_DOCS_DIR");
    // Export only, so that an export with other features (e.g., `full`) leaves the Python
    // wrappers of the installed build alone
    if let Ok(dir) = std::env::var("R4PM_EXPORT_DOCS_DIR") {
        export_docs_data(Path::new(&dir));
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let bindings_dir = Path::new(&manifest_dir).join("r4pm").join("bindings");

    if bindings_dir.exists() {
        fs::remove_dir_all(&bindings_dir).expect("Failed to remove old bindings");
    }
    fs::create_dir_all(&bindings_dir).expect("Failed to create bindings directory");

    let mut module_groups: BTreeMap<String, Vec<BindingMeta>> = BTreeMap::new();
    for func in funcs {
        module_groups
            .entry(rust_module_to_python(&func.module))
            .or_default()
            .push(func);
    }
    for funcs in module_groups.values_mut() {
        funcs.sort_by(|a, b| a.name.cmp(&b.name));
    }

    // A module that is also the parent of other modules (e.g., "bindings" next to
    // "bindings.slim_ocel_bindings") becomes a package with its functions in `_impl.py`
    let parent_modules: BTreeSet<String> = module_groups
        .keys()
        .filter(|path| {
            module_groups
                .keys()
                .any(|other| other != *path && other.starts_with(&format!("{}.", path)))
        })
        .cloned()
        .collect();

    let mut types = TypeRegistry::default();
    for (module_path, funcs) in &module_groups {
        generate_module_files(
            &bindings_dir,
            module_path,
            funcs,
            parent_modules.contains(module_path),
            &mut types,
        );
    }
    types.write(&bindings_dir);

    generate_submodule_inits(&bindings_dir, &module_groups);
    generate_bindings_init(&bindings_dir, &module_groups, &types);
}

/// Writes the docs data of this build for r4pm-docs. A `full` build (`r4pm-full`) writes the
/// binding metadata with shared schema definitions (`bindings-shared.json`) and the registry
/// kinds with their file formats and conversions (`registry-kinds.json`). A default build
/// (`r4pm`) writes the binding IDs and file formats it contains (`r4pm-contents.json`), so the
/// docs can mark what needs `r4pm-full`.
fn export_docs_data(dir: &Path) {
    use process_mining::bindings::RegistryItemKind;
    let kinds = RegistryItemKind::all_registered_kinds();
    let files = if std::env::var_os("CARGO_FEATURE_FULL").is_some() {
        let kinds: Vec<Value> = kinds
            .iter()
            .map(|k| {
                serde_json::json!({
                    "kind": k.name(),
                    "import_formats": k.known_import_formats(),
                    "export_formats": k.known_export_formats(),
                    "convertible_to": k.convertible_to().iter().map(|t| t.name()).collect::<Vec<_>>(),
                })
            })
            .collect();
        vec![
            (
                "bindings-shared.json",
                serde_json::to_value(process_mining::bindings::list_functions_meta_shared()),
            ),
            ("registry-kinds.json", serde_json::to_value(kinds)),
        ]
    } else {
        let extensions = |formats: Vec<process_mining::core::io::ExtensionWithMime>| {
            formats.into_iter().map(|f| f.extension).collect::<Vec<_>>()
        };
        let formats: serde_json::Map<String, Value> = kinds
            .iter()
            .map(|k| {
                let formats = serde_json::json!({
                    "import": extensions(k.known_import_formats()),
                    "export": extensions(k.known_export_formats()),
                });
                (k.name().to_string(), formats)
            })
            .collect();
        let bindings: Vec<String> = process_mining::bindings::list_functions_meta()
            .into_iter()
            .map(|f| f.id)
            .collect();
        vec![(
            "r4pm-contents.json",
            Ok(serde_json::json!({ "bindings": bindings, "formats": formats })),
        )]
    };
    fs::create_dir_all(dir).expect("Failed to create R4PM_EXPORT_DOCS_DIR");
    for (file, value) in files {
        let json = serde_json::to_string_pretty(&value.expect("Failed to serialize docs data"))
            .expect("Failed to serialize docs data");
        fs::write(dir.join(file), json).expect("Failed to write docs data");
    }
}

fn rust_module_to_python(rust_module: &str) -> String {
    rust_module
        .replace("process_mining::", "")
        .replace("::", ".")
}

fn module_docs_url(rust_module: &str) -> String {
    let path = rust_module.replace("::", "/");
    format!("{}/{}?v={}&lang={}", DOCS_BASE_URL, path, DOCS_VERSION, LANG)
}

fn function_docs_url(func: &BindingMeta) -> String {
    format!("{}#{}", module_docs_url(&func.module), func.name)
}

fn sanitize_identifier(name: &str) -> String {
    let s: String = name
        .replace("::", "_")
        .replace('+', "Plus")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if s.starts_with(|c: char| c.is_ascii_digit()) || PYTHON_KEYWORDS.contains(&s.as_str()) {
        format!("_{}", s)
    } else {
        s
    }
}

fn sanitize_name(name: &str) -> String {
    name.replace('-', "_").replace('+', "plus")
}

fn upper_camel(s: &str) -> String {
    s.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut chars = p.chars();
            let first = chars.next().unwrap().to_ascii_uppercase();
            std::iter::once(first).chain(chars).collect::<String>()
        })
        .collect()
}

/// Escape text for a triple-quoted Python docstring
fn doc_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace("\"\"\"", "\\\"\\\"\\\"")
}

/// The first paragraph of a doc comment, on one line (doc comments are hard-wrapped)
fn first_paragraph(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .take_while(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every TypedDict of all bindings, by name. Each is written once, to `_types.py`.
#[derive(Default)]
struct TypeRegistry {
    /// Name to (the schema it was generated from, its class definition)
    defs: BTreeMap<String, (Value, String)>,
}

impl TypeRegistry {
    /// The name for `schema`: the first of `bases` not taken by another schema, else the first
    /// one numbered. Also whether the type already exists under that name.
    fn name_for(&self, bases: &[String], schema: &Value) -> (String, bool) {
        let schema = &normalized(schema);
        let numbered = (2..).map(|n| format!("{}{}", bases[0], n));
        for candidate in bases.iter().cloned().chain(numbered) {
            match self.defs.get(&candidate) {
                None => return (candidate, false),
                Some((existing, _)) if existing == schema => return (candidate, true),
                Some(_) => {}
            }
        }
        unreachable!()
    }

    fn write(&self, bindings_dir: &Path) {
        let mut body = String::new();
        for (_, def) in self.defs.values() {
            body.push_str(def);
            body.push('\n');
        }
        let names: Vec<&String> = self.defs.keys().collect();
        let all = format!(
            "__all__ = [\n{}]\n",
            names
                .iter()
                .map(|n| format!("    \"{}\",\n", n))
                .collect::<String>()
        );
        let header = r#""""Types of the process_mining bindings.

This module is auto-generated. Every type is defined once here and imported by the binding modules.
"""
from __future__ import annotations
from typing import TYPE_CHECKING, Any, Dict, List, Literal, Optional, Tuple, Union
"#;
        // Annotations are strings at runtime, so only type checkers need `NotRequired`, which
        // `typing` lacks before Python 3.11
        let py = format!(
            "{header}from typing import TypedDict\n\nif TYPE_CHECKING:\n    from typing_extensions import NotRequired\n\n{body}{all}"
        );
        fs::write(bindings_dir.join("_types.py"), py).expect("Failed to write _types.py");
        let pyi = format!(
            "{header}from typing_extensions import NotRequired, TypedDict\n\n{body}{all}"
        );
        fs::write(bindings_dir.join("_types.pyi"), pyi).expect("Failed to write _types.pyi");
    }
}

/// Converts JSON schemas to Python type expressions, registering TypedDicts on the way
struct TypeGen<'a> {
    reg: &'a mut TypeRegistry,
    /// TypedDict names the current function refers to directly
    used: BTreeSet<String>,
}

impl TypeGen<'_> {
    /// A TypedDict for `schema`, named by the first of `bases` that is free
    fn typed_dict(&mut self, bases: &[&str], schema: &Value, root: &Value) -> String {
        let props = match schema.get("properties").and_then(|p| p.as_object()) {
            Some(p) => p,
            None => return "Dict[str, Any]".to_string(),
        };
        let bases: Vec<String> = bases.iter().map(|b| sanitize_identifier(b)).collect();
        let (name, exists) = self.reg.name_for(&bases, schema);
        self.used.insert(name.clone());
        if exists {
            return name;
        }
        if !bases.contains(&name) {
            println!(
                "cargo:warning=Two different types are named {}; the second is emitted as {}",
                bases[0], name
            );
        }
        // Reserve the name before recursing, for recursive types
        self.reg
            .defs
            .insert(name.clone(), (normalized(schema), String::new()));

        let required: BTreeSet<&str> = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        // Field types of nested TypedDicts live in `_types` too, so they are not "used" here
        let outer_used = std::mem::take(&mut self.used);
        let fields: Vec<(String, String, String)> = props
            .iter()
            .map(|(prop_name, prop_schema)| {
                let hint = format!("{}{}", name, upper_camel(prop_name));
                let ty = self.py_type(prop_schema, root, &hint);
                let ty = if required.contains(prop_name.as_str()) {
                    ty
                } else {
                    format!("NotRequired[{}]", ty)
                };
                let desc = prop_schema
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(|d| format!("  # {}", first_paragraph(d)))
                    .unwrap_or_default();
                (prop_name.clone(), ty, desc)
            })
            .collect();
        self.used = outer_used;

        let needs_functional = props
            .keys()
            .any(|k| PYTHON_KEYWORDS.contains(&k.as_str()) || !is_identifier(k));
        let class_def = if needs_functional {
            // Values are strings so that they are not evaluated at runtime, like the annotations
            // of the class syntax
            let lines: Vec<String> = fields
                .iter()
                .map(|(n, t, d)| format!("    {:?}: {:?},{}", n, t, d))
                .collect();
            format!(
                "{} = TypedDict(\"{}\", {{\n{}\n}})\n",
                name,
                name,
                lines.join("\n")
            )
        } else if fields.is_empty() {
            format!("class {}(TypedDict):\n    pass\n", name)
        } else {
            let lines: Vec<String> = fields
                .iter()
                .map(|(n, t, d)| format!("    {}: {}{}", n, t, d))
                .collect();
            format!("class {}(TypedDict):\n{}\n", name, lines.join("\n"))
        };
        self.reg
            .defs
            .insert(name.clone(), (normalized(schema), class_def));
        name
    }

    fn union(&mut self, variants: &[Value], root: &Value, hint: &str) -> String {
        let mut literals: Vec<String> = Vec::new();
        let mut types: Vec<String> = Vec::new();
        let mut nullable = false;
        for v in variants {
            if let Some(c) = v.get("const").and_then(|c| c.as_str()) {
                literals.push(format!("{:?}", c));
                continue;
            }
            if v.get("type").and_then(|t| t.as_str()) == Some("null") {
                nullable = true;
                continue;
            }
            // Externally tagged enum variant: `{"Variant": <data>}`
            if let Some((key, inner)) = single_key_variant(v) {
                let inner_ty = self.py_type(inner, root, &format!("{}{}", hint, upper_camel(key)));
                types.push(format!("Dict[Literal[{:?}], {}]", key, inner_ty));
                continue;
            }
            let variant_hint = match variant_tag(v) {
                Some(tag) => format!("{}{}", hint, upper_camel(tag)),
                None => hint.to_string(),
            };
            let t = self.py_type(v, root, &variant_hint);
            match t.strip_prefix("Optional[").and_then(|t| t.strip_suffix(']')) {
                Some(inner) => {
                    nullable = true;
                    types.push(inner.to_string());
                }
                None if t == "None" => nullable = true,
                None => types.push(t),
            }
        }
        if !literals.is_empty() {
            types.insert(0, format!("Literal[{}]", literals.join(", ")));
        }
        let mut seen = BTreeSet::new();
        types.retain(|t| seen.insert(t.clone()));
        let inner = match types.len() {
            0 => return "None".to_string(),
            1 => types.pop().unwrap(),
            _ => format!("Union[{}]", types.join(", ")),
        };
        if nullable {
            format!("Optional[{}]", inner)
        } else {
            inner
        }
    }

    fn py_type(&mut self, schema: &Value, root: &Value, hint: &str) -> String {
        if schema.as_bool() == Some(true) {
            return "Any".to_string();
        }
        if schema.get("x-registry-ref").is_some() {
            return "str".to_string();
        }
        if let Some(merged) = merged(schema, root) {
            return self.py_type(&merged, root, hint);
        }

        if let Some(def_name) = schema
            .get("$ref")
            .and_then(|r| r.as_str())
            .and_then(|r| r.strip_prefix("#/$defs/"))
        {
            let Some(def) = root.get("$defs").and_then(|d| d.get(def_name)) else {
                return "Any".to_string();
            };
            let title = def.get("title").and_then(|t| t.as_str());
            let def = &merged(def, root).unwrap_or_else(|| def.clone());
            if def.get("properties").is_some() {
                let bases: Vec<&str> = title.into_iter().chain([def_name]).collect();
                return self.typed_dict(&bases, def, root);
            }
            return self.py_type(def, root, title.unwrap_or(def_name));
        }

        for key in ["oneOf", "anyOf"] {
            if let Some(variants) = schema.get(key).and_then(|o| o.as_array()) {
                let hint = schema.get("title").and_then(|t| t.as_str()).unwrap_or(hint);
                return self.union(variants, root, hint);
            }
        }

        if let Some(types_arr) = schema.get("type").and_then(|t| t.as_array()) {
            let variants: Vec<Value> = types_arr
                .iter()
                .map(|t| {
                    let mut s = schema.clone();
                    s["type"] = t.clone();
                    s
                })
                .collect();
            return self.union(&variants, root, hint);
        }

        let ty = schema.get("type").and_then(|t| t.as_str()).or_else(|| {
            schema.get("properties").map(|_| "object")
        });
        match ty {
            Some("string") => {
                if let Some(c) = schema.get("const").and_then(|c| c.as_str()) {
                    return format!("Literal[{:?}]", c);
                }
                if let Some(vals) = schema.get("enum").and_then(|e| e.as_array()) {
                    let lits: Vec<String> = vals
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| format!("{:?}", s))
                        .collect();
                    if !lits.is_empty() {
                        return format!("Literal[{}]", lits.join(", "));
                    }
                }
                "str".to_string()
            }
            Some("integer") => "int".to_string(),
            Some("number") => "float".to_string(),
            Some("boolean") => "bool".to_string(),
            Some("null") => "None".to_string(),
            Some("array") => {
                if let Some(prefix) = schema.get("prefixItems").and_then(|p| p.as_array()) {
                    let items: Vec<String> = prefix
                        .iter()
                        .map(|i| self.py_type(i, root, hint))
                        .collect();
                    return format!("Tuple[{}]", items.join(", "));
                }
                let item = schema
                    .get("items")
                    .map(|i| self.py_type(i, root, hint))
                    .unwrap_or_else(|| "Any".to_string());
                format!("List[{}]", item)
            }
            Some("object") => {
                if schema.get("properties").is_some() {
                    let title = schema.get("title").and_then(|t| t.as_str());
                    let bases: Vec<&str> = title
                        .into_iter()
                        .chain([hint])
                        .filter(|b| !b.is_empty())
                        .collect();
                    if !bases.is_empty() {
                        return self.typed_dict(&bases, schema, root);
                    }
                }
                let value_schema = schema.get("additionalProperties").filter(|v| v.is_object()).or_else(|| {
                    schema
                        .get("patternProperties")
                        .and_then(|p| p.as_object())
                        .filter(|p| p.len() == 1)
                        .and_then(|p| p.values().next())
                });
                match value_schema {
                    Some(v) if v.is_object() => {
                        format!("Dict[str, {}]", self.py_type(v, root, hint))
                    }
                    _ => "Dict[str, Any]".to_string(),
                }
            }
            _ => "Any".to_string(),
        }
    }
}

/// A type's schema without what differs between the places it occurs in: a top-level argument
/// schema carries `$defs` and `$schema`, and a field's description replaces the type's own
fn normalized(schema: &Value) -> Value {
    let mut schema = schema.clone();
    if let Some(obj) = schema.as_object_mut() {
        for key in ["$defs", "$schema", "description", "title"] {
            obj.remove(key);
        }
    }
    schema
}

/// The value of the property that tags an enum variant (e.g., `{"type": {"const": "String"}}`)
fn variant_tag(schema: &Value) -> Option<&str> {
    schema
        .get("properties")?
        .as_object()?
        .values()
        .find_map(|p| p.get("const").and_then(|c| c.as_str()))
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with(|c: char| c.is_ascii_digit())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `schema` with its `allOf` parts and a `$ref` next to `properties` merged into one object, as
/// schemars emits internally tagged struct variants and flattened fields. A part that is a union
/// makes the result a union of merged objects. `None` if there is nothing to merge.
fn merged(schema: &Value, root: &Value) -> Option<Value> {
    let obj = schema.as_object()?;
    let ref_with_props = obj.contains_key("$ref") && obj.contains_key("properties");
    if !ref_with_props && !obj.contains_key("allOf") {
        return None;
    }
    let mut base = obj.clone();
    let mut parts: Vec<Value> = base
        .remove("allOf")
        .and_then(|a| match a {
            Value::Array(parts) => Some(parts),
            _ => None,
        })
        .unwrap_or_default();
    if ref_with_props {
        parts.extend(
            base.remove("$ref")
                .map(|r| serde_json::json!({ "$ref": r })),
        );
    }
    // A lone `allOf: [{"$ref": ...}]` is how a described field refers to its type
    if parts.len() == 1 && !base.contains_key("properties") {
        let part = parts.pop().unwrap();
        return Some(merged(&part, root).unwrap_or(part));
    }

    let mut alternatives = vec![base];
    for part in parts {
        let part = resolve_ref(&part, root);
        let part = merged(&part, root).unwrap_or(part);
        let variants: Vec<Value> = match ["oneOf", "anyOf"]
            .iter()
            .find_map(|k| part.get(*k).and_then(|v| v.as_array()))
        {
            Some(variants) => variants
                .iter()
                .map(|v| {
                    let v = resolve_ref(v, root);
                    merged(&v, root).unwrap_or(v)
                })
                .collect(),
            None => vec![part],
        };
        alternatives = alternatives
            .iter()
            .flat_map(|alt| variants.iter().map(move |v| merge_object(alt, v)))
            .collect();
    }
    let mut alternatives: Vec<Value> = alternatives.into_iter().map(Value::Object).collect();
    if alternatives.len() == 1 {
        alternatives.pop()
    } else {
        Some(serde_json::json!({ "oneOf": alternatives }))
    }
}

/// The `$defs` entry a bare `{"$ref": ...}` points to, else `schema` itself
fn resolve_ref(schema: &Value, root: &Value) -> Value {
    let target = schema
        .get("$ref")
        .and_then(|r| r.as_str())
        .and_then(|r| r.strip_prefix("#/$defs/"))
        .and_then(|name| root.get("$defs")?.get(name));
    match target {
        Some(def) if schema.get("properties").is_none() => resolve_ref(def, root),
        _ => schema.clone(),
    }
}

/// `base` with the properties and required properties of `part` added. Those of `base` win, as
/// they hold the tag of an internally tagged variant.
fn merge_object(
    base: &serde_json::Map<String, Value>,
    part: &Value,
) -> serde_json::Map<String, Value> {
    let mut out = base.clone();
    out.insert("type".to_string(), "object".into());
    if let Some(props) = part.get("properties").and_then(|p| p.as_object()) {
        let out_props = out
            .entry("properties")
            .or_insert_with(|| Value::Object(Default::default()));
        if let Some(out_props) = out_props.as_object_mut() {
            for (k, v) in props {
                out_props.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
    }
    if let Some(req) = part.get("required").and_then(|r| r.as_array()) {
        let out_req = out
            .entry("required")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(out_req) = out_req.as_array_mut() {
            for r in req {
                if !out_req.contains(r) {
                    out_req.push(r.clone());
                }
            }
        }
    }
    out
}

/// The key and value schema of an object with exactly one, required property that is not a tag
fn single_key_variant(schema: &Value) -> Option<(&str, &Value)> {
    let props = schema.get("properties")?.as_object()?;
    if props.len() != 1
        || schema.get("title").is_some()
        || schema.get("$ref").is_some()
        || schema.get("allOf").is_some()
    {
        return None;
    }
    let (key, inner) = props.iter().next()?;
    if inner.get("const").is_some_and(|c| c.is_string()) {
        return None;
    }
    let required = schema.get("required")?.as_array()?;
    (required.len() == 1 && required[0].as_str() == Some(key.as_str()))
        .then_some((key.as_str(), inner))
}

/// Python types of one binding's arguments and result
struct FuncTypes {
    args: Vec<String>,
    ret: String,
    /// Registry kind for each argument passed as a registry reference
    registry_args: BTreeMap<String, String>,
    /// Arguments that must name a stored item, as the binding changes it in place
    mut_args: BTreeSet<String>,
    /// Registry kind the binding stores under its required `output_id`, next to returning `ret`
    stores_kind: Option<String>,
    /// Registry kind of the result, if it is stored in the registry
    returns_kind: Option<String>,
    /// Whether the wrapper returns the result as `Svg` (see `SVG_MEDIA_TYPE`)
    returns_svg: bool,
    used: BTreeSet<String>,
}

fn registry_kind(schema: &Value) -> Option<String> {
    schema
        .get("x-registry-ref")
        .and_then(|r| r.as_str())
        .map(str::to_string)
}

fn is_mut_arg(schema: &Value) -> bool {
    schema.get("x-registry-mut").and_then(|m| m.as_bool()) == Some(true)
}

/// The keyword-only argument every wrapper adds, so no binding may have one by this name
const PROGRESS_ARG: &str = "progress";

/// Media type of the bindings returning an SVG document, which their wrappers return as
/// `r4pm.svg.Svg`
const SVG_MEDIA_TYPE: &str = "image/svg+xml";

fn handle_type(kind: &str) -> String {
    format!("handles.{}", kind)
}

/// Registry kinds that bindings otherwise take as JSON, e.g., `AcceptingPetriNet`
fn model_kinds() -> BTreeSet<String> {
    use process_mining::bindings::RegistryItemKind;
    let built_in: Vec<&str> = RegistryItemKind::all_kinds().iter().map(|k| k.name()).collect();
    RegistryItemKind::all_registered_kinds()
        .iter()
        .map(|k| k.name())
        .filter(|name| !built_in.contains(name))
        .map(str::to_string)
        .collect()
}

/// The JSON schema of each model kind, taken from a binding that passes the model as JSON, so a
/// binding taking the model as a registry reference can also accept it inline
fn model_schemas(model_kinds: &BTreeSet<String>) -> BTreeMap<String, Value> {
    let mut schemas = BTreeMap::new();
    for func in process_mining::bindings::list_functions_meta() {
        let schemas_of_func = func.args.iter().map(|(_, s)| s.clone()).chain([func.return_type]);
        for schema in schemas_of_func {
            if let Some(title) = schema.get("title").and_then(|t| t.as_str()) {
                if model_kinds.contains(title) && schema.get("x-registry-ref").is_none() {
                    schemas.entry(title.to_string()).or_insert(schema);
                }
            }
        }
    }
    schemas
}

fn func_types(func: &BindingMeta, reg: &mut TypeRegistry) -> FuncTypes {
    let model_kinds = model_kinds();
    let model_schemas = model_schemas(&model_kinds);
    let mut gen = TypeGen {
        reg,
        used: BTreeSet::new(),
    };
    let mut registry_args = BTreeMap::new();
    let mut mut_args = BTreeSet::new();
    let args = func
        .args
        .iter()
        .map(|(name, schema)| match registry_kind(schema) {
            Some(kind) => {
                registry_args.insert(name.clone(), kind.clone());
                if is_mut_arg(schema) {
                    mut_args.insert(name.clone());
                }
                match model_schemas.get(&kind) {
                    Some(model) if !is_mut_arg(schema) => {
                        let ty = gen.py_type(model, model, &kind);
                        format!("Union[{}, {}, str]", handle_type(&kind), ty)
                    }
                    _ => format!("Union[{}, str]", handle_type(&kind)),
                }
            }
            None => {
                let hint = format!("{}{}", upper_camel(&func.name), upper_camel(name));
                let ty = gen.py_type(schema, schema, &hint);
                // A model kept in the registry is passed as JSON by `r4pm._call`
                match schema.get("title").and_then(|t| t.as_str()) {
                    Some(title) if model_kinds.contains(title) => {
                        format!("Union[{}, {}]", ty, handle_type(title))
                    }
                    _ => ty,
                }
            }
        })
        .collect();
    let returns_kind = registry_kind(&func.return_type);
    let mut ret = match &returns_kind {
        Some(kind) => handle_type(kind),
        None => {
            let hint = format!("{}Result", upper_camel(&func.name));
            gen.py_type(&func.return_type, &func.return_type, &hint)
        }
    };
    let returns_svg = func.return_type.get("contentMediaType").and_then(Value::as_str)
        == Some(SVG_MEDIA_TYPE);
    if returns_svg {
        assert_eq!(ret, "str", "SVG binding {} does not return a string", func.name);
        ret = "Svg".to_string();
    }
    let stores_kind = func.args.iter().find_map(|(name, schema)| {
        (name == "output_id")
            .then(|| schema.get("x-registry-out")?.as_str().map(str::to_string))
            .flatten()
    });
    FuncTypes {
        args,
        ret,
        registry_args,
        mut_args,
        stores_kind,
        returns_kind,
        returns_svg,
        used: gen.used,
    }
}

struct Signature {
    params: String,
    doc: String,
}

fn signature(func: &BindingMeta, types: &FuncTypes) -> Signature {
    let required_args: BTreeSet<&str> = func.required_args.iter().map(|s| s.as_str()).collect();
    // Python requires all required (non-default) params before any optional ones
    let mut required_params = Vec::new();
    let mut optional_params = Vec::new();
    let mut doc_args = Vec::new();
    for (i, (arg_name, arg_schema)) in func.args.iter().enumerate() {
        let arg_type = &types.args[i];
        if required_args.contains(arg_name.as_str()) {
            required_params.push(format!("{}: {}", arg_name, arg_type));
        } else {
            let ty = if arg_type.starts_with("Optional[") {
                arg_type.clone()
            } else {
                format!("Optional[{}]", arg_type)
            };
            optional_params.push(format!("{}: {} = None", arg_name, ty));
        }
        let stored = types
            .stores_kind
            .as_ref()
            .filter(|_| arg_name == "output_id");
        let desc = if let Some(kind) = stored {
            format!(
                "Registry ID to store the resulting {} under, replacing any item stored there",
                kind
            )
        } else if let Some(kind) = types.registry_args.get(arg_name) {
            if types.mut_args.contains(arg_name) {
                format!(
                    "{} handle or registry ID of a stored item, changed in place",
                    kind
                )
            } else {
                format!(
                    "{} handle, registry ID, or path to a file to load it from",
                    kind
                )
            }
        } else if let Some(desc) = arg_schema.get("description").and_then(|d| d.as_str()) {
            first_paragraph(desc)
        } else {
            arg_type.clone()
        };
        doc_args.push(format!("        {}: {}", arg_name, desc));
    }
    doc_args.push(format!(
        "        {}: Progress display for this call: a callback, \"tqdm\", or False for none \
         (default: the global setting, see `r4pm.set_progress`)",
        PROGRESS_ARG
    ));

    let mut doc_lines: Vec<String> = func.docs.iter().map(|s| format!("    {}", s)).collect();
    doc_lines.push(String::new());
    doc_lines.push(format!("    See: {}", function_docs_url(func)));
    doc_lines.push(String::new());
    doc_lines.push("    Args:".to_string());
    doc_lines.extend(doc_args);
    doc_lines.push(String::new());
    doc_lines.push("    Returns:".to_string());
    match (&types.returns_kind, &types.stores_kind) {
        (Some(kind), _) => doc_lines.push(format!("        Handle to the resulting {}", kind)),
        (None, Some(kind)) => doc_lines.push(format!(
            "        {} (the resulting {} is stored under `output_id`)",
            types.ret, kind
        )),
        (None, None) => doc_lines.push(format!("        {}", types.ret)),
    }

    assert!(
        !func.args.iter().any(|(name, _)| name == PROGRESS_ARG),
        "Binding {} has an argument named `{}`, which clashes with the progress option",
        func.id,
        PROGRESS_ARG
    );
    let progress_param = format!("{}: ProgressOption = None", PROGRESS_ARG);
    Signature {
        params: required_params
            .into_iter()
            .chain(optional_params)
            .chain(["*".to_string(), progress_param])
            .collect::<Vec<_>>()
            .join(", "),
        doc: doc_escape(&doc_lines.join("\n")),
    }
}

fn generate_function_wrapper(func: &BindingMeta, types: &FuncTypes) -> String {
    let py_name = sanitize_name(&func.name);
    let sig = signature(func, types);
    let required_args: BTreeSet<&str> = func.required_args.iter().map(|s| s.as_str()).collect();
    let mut body = vec!["    args: Dict[str, Any] = {}".to_string()];
    for (arg_name, _) in &func.args {
        if required_args.contains(arg_name.as_str()) {
            body.push(format!("    args[{:?}] = {}", arg_name, arg_name));
        } else {
            body.push(format!("    if {} is not None:", arg_name));
            body.push(format!("        args[{:?}] = {}", arg_name, arg_name));
        }
    }
    let registry_args: Vec<String> = types
        .registry_args
        .keys()
        .map(|a| format!("{:?}, ", a))
        .collect();
    let returns = match &types.returns_kind {
        Some(kind) => format!("{:?}", kind),
        None => "None".to_string(),
    };
    let (open, close) = if types.returns_svg {
        ("Svg(", ")")
    } else {
        ("", "")
    };
    format!(
        r#"def {py_name}({params}) -> {ret}:
    """
{doc}
    """
{body}
    return {open}_call(
        {id:?}, args, registry_args=({registry_args}), returns={returns}, progress={progress}
    ){close}"#,
        params = sig.params,
        ret = types.ret,
        doc = sig.doc,
        body = body.join("\n"),
        id = func.id,
        registry_args = registry_args.concat(),
        progress = PROGRESS_ARG,
    )
}

fn generate_function_stub(func: &BindingMeta, types: &FuncTypes) -> String {
    let sig = signature(func, types);
    format!(
        r#"def {}({}) -> {}:
    """
{}
    """
    ..."#,
        sanitize_name(&func.name),
        sig.params,
        types.ret,
        sig.doc
    )
}

fn type_imports(used: &BTreeSet<String>) -> String {
    if used.is_empty() {
        return String::new();
    }
    format!(
        "from {} import (\n{})\n",
        TYPES_MODULE,
        used.iter()
            .map(|n| format!("    {} as {},\n", n, n))
            .collect::<String>()
    )
}

fn generate_module_files(
    bindings_dir: &Path,
    module_path: &str,
    functions: &[BindingMeta],
    is_package_parent: bool,
    reg: &mut TypeRegistry,
) {
    let parts: Vec<&str> = module_path.split('.').collect();
    // A module that is also a package parent cannot be `foo.py` next to a `foo/` directory (the
    // directory shadows the file), so its functions go to `foo/_impl.py`
    let (module_dir, file_name) = if is_package_parent {
        (bindings_dir.join(parts.join("/")), "_impl".to_string())
    } else if parts.len() > 1 {
        (
            bindings_dir.join(parts[..parts.len() - 1].join("/")),
            parts.last().unwrap().to_string(),
        )
    } else {
        (bindings_dir.to_path_buf(), parts[0].to_string())
    };
    fs::create_dir_all(&module_dir).expect("Failed to create module directory");

    let rust_module = format!("process_mining::{}", module_path.replace('.', "::"));
    let docs_url = module_docs_url(&rust_module);

    let all_types: Vec<FuncTypes> = functions.iter().map(|f| func_types(f, reg)).collect();
    let used: BTreeSet<String> = all_types.iter().flat_map(|t| t.used.clone()).collect();
    let svg_import = if all_types.iter().any(|t| t.returns_svg) {
        "from r4pm.svg import Svg\n"
    } else {
        ""
    };
    let names: Vec<String> = functions.iter().map(|f| sanitize_name(&f.name)).collect();
    let all = format!(
        "__all__ = [\n{}]\n",
        names
            .iter()
            .map(|n| format!("    {:?},\n", n))
            .collect::<String>()
    );

    let header = |what: &str, extra_imports: &str| {
        format!(
            r#""""{module_path} - {what} for the process_mining Rust library.

Rust Documentation: {docs_url}

This module is auto-generated.
"""
from __future__ import annotations
from typing import Any, Dict, List, Literal, Optional, Tuple, Union

from r4pm import handles
from r4pm._progress import ProgressOption
{svg_import}{extra_imports}{types}
"#,
            types = type_imports(&used),
        )
    };

    let mut py = header("Python bindings", "from r4pm._call import call as _call\n");
    for (func, types) in functions.iter().zip(&all_types) {
        py.push_str(&generate_function_wrapper(func, types));
        py.push_str("\n\n\n");
    }
    py.push_str(&all);
    fs::write(module_dir.join(format!("{}.py", file_name)), py)
        .expect("Failed to write .py file");

    let mut pyi = header("Type stubs", "");
    for (func, types) in functions.iter().zip(&all_types) {
        pyi.push_str(&generate_function_stub(func, types));
        pyi.push_str("\n\n");
    }
    pyi.push_str(&all);
    fs::write(module_dir.join(format!("{}.pyi", file_name)), pyi)
        .expect("Failed to write .pyi file");
}

/// Generate __init__.py files for intermediate directories
fn generate_submodule_inits(
    bindings_dir: &Path,
    module_groups: &BTreeMap<String, Vec<BindingMeta>>,
) {
    // Directory to (subpackages, module files)
    let mut dir_contents: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> =
        BTreeMap::new();

    for module_path in module_groups.keys() {
        let parts: Vec<&str> = module_path.split('.').collect();
        let parent = parts[..parts.len() - 1].join("/");
        dir_contents
            .entry(parent)
            .or_default()
            .1
            .insert(parts.last().unwrap().to_string());
        for i in 1..parts.len() {
            dir_contents
                .entry(parts[..i - 1].join("/"))
                .or_default()
                .0
                .insert(parts[i - 1].to_string());
        }
    }

    for (dir_path, (submodules, files)) in &dir_contents {
        if dir_path.is_empty() {
            continue;
        }
        let target_dir = bindings_dir.join(dir_path);
        fs::create_dir_all(&target_dir).expect("Failed to create submodule directory");
        let has_impl = target_dir.join("_impl.py").exists();

        let mut imports = String::new();
        for submod in submodules {
            imports.push_str(&format!("from . import {} as {}\n", submod, submod));
        }
        // A file module that is also a subpackage was already imported as one above
        for file_mod in files.difference(submodules) {
            imports.push_str(&format!("from .{} import *\n", file_mod));
        }
        if has_impl {
            imports.push_str("from ._impl import *\n");
        }

        // Explicit `__all__`, with the star-imported functions, so that type checkers see them as
        // re-exported
        let mut all_items: Vec<String> = submodules.iter().cloned().collect();
        let module_prefix = dir_path.replace('/', ".");
        for file_mod in files.difference(submodules) {
            let path = format!("{}.{}", module_prefix, file_mod);
            if let Some(funcs) = module_groups.get(&path) {
                all_items.extend(funcs.iter().map(|f| sanitize_name(&f.name)));
            }
        }
        if has_impl {
            if let Some(funcs) = module_groups.get(&module_prefix) {
                all_items.extend(funcs.iter().map(|f| sanitize_name(&f.name)));
            }
        }
        let content = format!(
            "\"\"\"Auto-generated module.\"\"\"\n\n{}\n__all__ = [\n{}]\n",
            imports,
            all_items
                .iter()
                .map(|n| format!("    {:?},\n", n))
                .collect::<String>()
        );
        fs::write(target_dir.join("__init__.py"), &content)
            .expect("Failed to write submodule __init__.py");
        fs::write(target_dir.join("__init__.pyi"), &content)
            .expect("Failed to write submodule __init__.pyi");
    }
}

/// Generate the main bindings/__init__.py
fn generate_bindings_init(
    bindings_dir: &Path,
    module_groups: &BTreeMap<String, Vec<BindingMeta>>,
    types: &TypeRegistry,
) {
    let top_modules: BTreeSet<String> = module_groups
        .keys()
        .map(|p| p.split('.').next().unwrap().to_string())
        .collect();

    // Top-level names must be unique: modules first, then functions, then types
    let mut taken: BTreeSet<String> = top_modules.iter().cloned().collect();
    let mut funcs: Vec<(String, String)> = module_groups
        .iter()
        .flat_map(|(module, fs)| fs.iter().map(move |f| (sanitize_name(&f.name), module.clone())))
        .collect();
    funcs.sort();
    let mut func_imports = String::new();
    let mut exported: Vec<String> = top_modules.iter().cloned().collect();
    for (name, module) in &funcs {
        if !taken.insert(name.clone()) {
            println!(
                "cargo:warning=Binding {}.{} is not re-exported at the top level of \
                 r4pm.bindings: the name is taken",
                module, name
            );
            continue;
        }
        func_imports.push_str(&format!("from .{} import {} as {}\n", module, name, name));
        exported.push(name.clone());
    }
    let mut type_imports = String::new();
    for name in types.defs.keys() {
        if taken.insert(name.clone()) {
            type_imports.push_str(&format!("from ._types import {} as {}\n", name, name));
            exported.push(name.clone());
        }
    }

    let bindings_docs_url = module_docs_url("process_mining::bindings");
    let content = format!(
        r#""""r4pm.bindings - Python bindings for the process_mining Rust library.

All functions are auto-generated from Rust function metadata. Arguments that refer to registry
items accept handles (see `r4pm.handles`), registry IDs, or file paths; results stored in the
registry are returned as handles.

Rust Documentation: {docs_url}

Usage:
    from r4pm.bindings import discover_dfg
    # or access via submodules:
    from r4pm.bindings.discovery.case_centric.dfg import discover_dfg
"""
from __future__ import annotations

{modules}
{func_imports}
{type_imports}
__all__ = [
{all}]
"#,
        docs_url = bindings_docs_url,
        modules = top_modules
            .iter()
            .map(|m| format!("from . import {} as {}\n", m, m))
            .collect::<String>(),
        all = exported
            .iter()
            .map(|n| format!("    {:?},\n", n))
            .collect::<String>(),
    );
    fs::write(bindings_dir.join("__init__.py"), &content)
        .expect("Failed to write bindings __init__.py");
    fs::write(bindings_dir.join("__init__.pyi"), &content)
        .expect("Failed to write bindings __init__.pyi");
}

/// Write enabled features to `OUT_DIR/enabled_features.rs`.
///
/// Cargo sets `CARGO_FEATURE_<NAME>` upper-cased with `-` as `_`; this crate's names use `-`.
fn generate_enabled_features() {
    let mut features: Vec<String> = std::env::vars()
        .filter_map(|(key, _)| key.strip_prefix("CARGO_FEATURE_").map(str::to_string))
        .map(|name| name.to_lowercase().replace('_', "-"))
        .filter(|name| name != "default")
        .collect();
    features.sort();

    let entries = features
        .iter()
        .map(|f| format!("    \"{f}\","))
        .collect::<Vec<_>>()
        .join("\n");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("enabled_features.rs");
    fs::write(
        out,
        format!("pub const ENABLED_FEATURES: &[&str] = &[\n{entries}\n];\n"),
    )
    .expect("Failed to write enabled_features.rs");
}
