use std::time::Instant;

use process_mining::bindings::{
    call, get_fn_binding, list_functions_meta, resolve_argument, AppState, RegistryItem,
    RegistryItemKind,
};
use process_mining::core::event_data::case_centric::{
    dataframe::{convert_dataframe_to_log, convert_log_to_dataframe},
    xes::{export_xes_event_log_to_path, import_xes_path, XESImportOptions, XESOuterLogData},
};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyDict, PyList},
};
use pyo3_polars::PyDataFrame;
use serde_json::Value as JsonValue;

use crate::ocel::{
    OCEL2DataFrames, OCEL2DataFramesRef, export_ocel_rs, import_ocel_json_rs, import_ocel_rs, import_ocel_xml_rs
};

mod ocel;
///
/// Import an XES event log
///
/// Returns a tuple of a Polars [DataFrame] for the event data and a json-encoding of  all log attributes/extensions/classifiers
///
/// * `path` - The filepath of the .xes or .xes.gz file to import
/// * `date_format` - Optional date format to use for parsing <date> tags (See https://docs.rs/chrono/latest/chrono/format/strftime/index.html)
/// * `print_debug` - Optional flag to enable debug print outputs
///
///
#[pyfunction]
fn import_xes_rs(
    path: String,
    date_format: Option<String>,
    print_debug: Option<bool>,
) -> PyResult<(PyDataFrame, String)> {
    if print_debug.is_some_and(|a| a) {
        println!("Starting XES Import");
    }
    let start_now = Instant::now();
    let mut now = Instant::now();
    let log = import_xes_path(
        &path,
        XESImportOptions {
            date_format,
            ..Default::default()
        },
    )
    .unwrap();
    if print_debug.is_some_and(|a| a) {
        println!("Importing XES Log took {:.2?}", now.elapsed());
    }
    now = Instant::now();
    let other_data = XESOuterLogData {
        log_attributes: log.attributes.clone(),
        extensions: log.extensions.clone().unwrap_or_default().clone(),
        classifiers: log.classifiers.clone().unwrap_or_default().clone(),
        global_trace_attrs: log.global_trace_attrs.clone().unwrap_or_default(),
        global_event_attrs: log.global_event_attrs.clone().unwrap_or_default(),
    };
    let converted_log = convert_log_to_dataframe(&log, print_debug.unwrap_or_default()).unwrap();
    if print_debug.is_some_and(|a| a) {
        println!("Finished Converting Log; Took {:.2?}", now.elapsed());
    }
    if print_debug.is_some_and(|a| a) {
        println!("Total duration: {:.2?}", start_now.elapsed());
    }
    Ok((
        PyDataFrame(converted_log),
        serde_json::to_string(&other_data).unwrap(),
    ))
}

#[pyfunction]
fn export_xes_rs(df: PyDataFrame, path: String) -> PyResult<()> {
    let df: polars::frame::DataFrame = df.into();
    let log = convert_dataframe_to_log(&df)
        .map_err(|e| PyTypeError::new_err(format!("Failed to convert dataframe to log: {e:?}")))?;

    export_xes_event_log_to_path(&log, path)
        .map_err(|e| PyTypeError::new_err(format!("Failed to export XES: {e:?}")))
}

/// Python Module
#[pymodule]
fn r4pm(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(import_xes_rs, m)?)?;
    m.add_function(wrap_pyfunction!(export_xes_rs, m)?)?;
    m.add_function(wrap_pyfunction!(import_ocel_xml_rs, m)?)?;
    m.add_function(wrap_pyfunction!(import_ocel_json_rs, m)?)?;
    m.add_function(wrap_pyfunction!(import_ocel_rs, m)?)?;
    m.add_function(wrap_pyfunction!(export_ocel_rs, m)?)?;

    // Bindings support
    // list_bindings
    m.add_function(wrap_pyfunction!(list_bindings, m)?)?;
    // call_binding
    m.add_function(wrap_pyfunction!(call_binding, m)?)?;
    // import_item
    m.add_function(wrap_pyfunction!(import_item, m)?)?;
    // convert_item
    m.add_function(wrap_pyfunction!(convert_item, m)?)?;
    // export_item
    m.add_function(wrap_pyfunction!(export_item, m)?)?;
    // item_to_df
    m.add_function(wrap_pyfunction!(item_to_df, m)?)?;
    // list_items
    m.add_function(wrap_pyfunction!(list_items, m)?)?;
    // remove_item
    m.add_function(wrap_pyfunction!(remove_item, m)?)?;
    // import_item_from_df
    m.add_function(wrap_pyfunction!(import_item_from_df, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;

    Ok(())
}

/// List all available binding functions
///
/// Returns a list of dictionaries containing metadata about each function
/// Each dictionary includes: name, description, arguments, and return type
#[pyfunction]
fn list_bindings() -> PyResult<Vec<PyObject>> {
    let functions = list_functions_meta();
    Python::with_gil(|py| {
        functions
            .iter()
            .map(|f| {
                let dict = PyDict::new(py);
                dict.set_item("id", &f.id)?;
                dict.set_item("name", &f.name)?;
                dict.set_item("description", &f.docs)?;
                dict.set_item("module", &f.module)?;

                // Convert arguments to Python-friendly format
                let args = PyList::empty(py);
                for (arg_name, arg_schema) in &f.args {
                    let arg_dict = PyDict::new(py);
                    arg_dict.set_item("name", arg_name)?;
                    arg_dict.set_item("schema", serde_json::to_string(arg_schema).unwrap())?;
                    args.append(arg_dict)?;
                }
                dict.set_item("arguments", args)?;

                // Add required arguments
                let required_args_list = PyList::new(py, f.required_args.iter())?;
                dict.set_item("required_arguments", required_args_list)?;

                dict.set_item(
                    "return_schema",
                    serde_json::to_string(&f.return_type).unwrap(),
                )?;

                Ok(dict.into())
            })
            .collect()
    })
}

/// Call a binding function by ID with provided arguments
///
/// # Arguments
/// * `function_id` - ID of the function to call (e.g., "process_mining::bindings::test_some_inputs")
/// * `args_json` - JSON string containing the function arguments
///
/// # Returns
/// JSON string containing the result (either a value or a registry item ID)
#[pyfunction]
fn call_binding(function_id: String, args_json: String, py: Python<'_>) -> PyResult<String> {
    // Get the global app state
    let state = get_or_create_app_state(py)?;

    // Parse arguments
    let mut args: JsonValue = serde_json::from_str(&args_json)
        .map_err(|e| PyTypeError::new_err(format!("Invalid JSON arguments: {}", e)))?;

    // Get the binding function
    let binding = get_fn_binding(&function_id)
        .ok_or_else(|| PyTypeError::new_err(format!("Function '{}' not found", function_id)))?;

    // Resolve arguments (automatic type conversion for registry items)
    if let JsonValue::Object(ref mut args_map) = args {
        // Get the parameters schema from the binding
        let params_schema: Vec<(String, JsonValue)> = (binding.args)();

        for (param_name, param_schema) in params_schema.iter() {
            if let Some(arg_value) = args_map.get_mut(param_name) {
                // Resolve the argument using the shared AppState
                match resolve_argument(param_name, arg_value.clone(), param_schema, &state) {
                    Ok(resolved_value) => {
                        *arg_value = resolved_value;
                    }
                    Err(_) => {
                        // If resolution fails, leave the original value
                        // The call() function will handle the error
                    }
                }
            }
        }
    }

    // Call the function
    let result = call(binding, &args, &state)
        .map_err(|e| PyTypeError::new_err(format!("Function call failed: {}", e)))?;

    // Return result as JSON
    serde_json::to_string(&result)
        .map_err(|e| PyTypeError::new_err(format!("Failed to serialize result: {}", e)))
}

/// Load a registry item from a file path
///
/// # Arguments
/// * `item_type` - Type of item to load ("EventLog", "OCEL", "IndexLinkedOCEL", "SlimLinkedOCEL", "EventLogActivityProjection")
/// * `file_path` - Path to the file to load
/// * `item_id` - Optional ID to assign to the loaded item (auto-generated if not provided)
///
/// # Returns
/// The ID of the loaded item in the registry
#[pyfunction]
#[pyo3(signature = (item_type, file_path, item_id=None))]
fn import_item(
    item_type: String,
    file_path: String,
    item_id: Option<String>,
    py: Python<'_>,
) -> PyResult<String> {
    let state = get_or_create_app_state(py)?;

    // Parse the item type
    let kind: RegistryItemKind = item_type
        .parse()
        .map_err(|e| PyTypeError::new_err(format!("Invalid item type '{}': {}", item_type, e)))?;

    // Load the item using the process_mining API
    let item = RegistryItem::load_from_path(&kind, &file_path).map_err(|e| {
        PyTypeError::new_err(format!(
            "Failed to load {} from {}: {}",
            item_type, file_path, e
        ))
    })?;

    let id = item_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    state.items.write().unwrap().insert(id.clone(), item);

    Ok(id)
}

/// Convert a registry item to another type
///
/// # Arguments
/// * `item_id` - ID of the item in the registry
/// * `target_type` - Target type to convert to ("EventLog", "OCEL", "IndexLinkedOCEL", "SlimLinkedOCEL", "EventLogActivityProjection")
/// * `new_item_id` - Optional ID for the converted item (auto-generated if not provided)
///
/// # Returns
/// The ID of the newly created converted item in the registry
#[pyfunction]
#[pyo3(signature = (item_id, target_type, new_item_id=None))]
fn convert_item(
    item_id: String,
    target_type: String,
    new_item_id: Option<String>,
    py: Python<'_>,
) -> PyResult<String> {
    let state = get_or_create_app_state(py)?;

    // Parse the target type
    let target_kind: RegistryItemKind = target_type.parse().map_err(|e| {
        PyTypeError::new_err(format!("Invalid target type '{}': {}", target_type, e))
    })?;

    // Convert the item (needs to be done while holding the read lock)
    let converted_item = {
        let state_guard = state.items.read().unwrap();
        let source_item = state_guard
            .get(&item_id)
            .ok_or_else(|| PyTypeError::new_err(format!("Item not found: {}", item_id)))?;

        source_item
            .convert(target_kind)
            .map_err(|e| PyTypeError::new_err(format!("Failed to convert item: {}", e)))?
    };

    let new_id = new_item_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    state
        .items
        .write()
        .unwrap()
        .insert(new_id.clone(), converted_item);

    Ok(new_id)
}

/// Export a registry item to a file path
///
/// # Arguments
/// * `item_id` - ID of the item to export
/// * `file_path` - Path where the file should be written
///
/// # Returns
/// Unit on success
#[pyfunction]
fn export_item(item_id: String, file_path: String, py: Python<'_>) -> PyResult<()> {
    let state = get_or_create_app_state(py)?;
    let state_guard = state.items.read().unwrap();

    let item = state_guard
        .get(&item_id)
        .ok_or_else(|| PyTypeError::new_err(format!("Item not found: {}", item_id)))?;

    item.export_to_path(&file_path)
        .map_err(|e| PyTypeError::new_err(format!("Failed to export item: {}", e)))?;

    Ok(())
}

/// Get a registry item as a Polars DataFrame
///
/// # Arguments
/// * `item_id` - ID of the item in the registry
///
/// # Returns
/// For EventLog: DataFrame with event data
/// For OCEL: Dictionary with multiple DataFrames (events, objects, o2o, relations, object_changes)
#[pyfunction]
fn item_to_df(item_id: String, py: Python<'_>) -> PyResult<PyObject> {
    let state = get_or_create_app_state(py)?;
    let state_guard = state.items.read().unwrap();

    let item = state_guard
        .get(&item_id)
        .ok_or_else(|| PyTypeError::new_err(format!("Item not found: {}", item_id)))?;

    match item {
        RegistryItem::EventLog(log) => {
            let df = convert_log_to_dataframe(log, false)
                .map_err(|e| PyTypeError::new_err(format!("Failed to convert log: {}", e)))?;
            let py_df = PyDataFrame(df);
            Ok(py_df.into_pyobject(py)?.into_any().unbind())
        }
        RegistryItem::OCEL(ocel) => {
            let ocel_dfs = crate::ocel::ocel2_to_df(ocel);
            let result_map = crate::ocel::ocel_dfs_to_py(ocel_dfs);
            let py_dict = PyDict::new(py);
            for (k, v) in result_map {
                py_dict.set_item(k, v.into_pyobject(py)?)?;
            }
            Ok(py_dict.into_any().unbind())
        }
        _ => Err(PyTypeError::new_err(
            "This item type cannot be converted to DataFrame",
        )),
    }
}

/// List all items currently in the registry
///
/// # Returns
/// List of dictionaries with 'id' and 'type' fields
#[pyfunction]
fn list_items(py: Python<'_>) -> PyResult<Vec<PyObject>> {
    let state = get_or_create_app_state(py)?;
    let state_guard = state.items.read().unwrap();

    Python::with_gil(|py| {
        state_guard
            .iter()
            .map(|(id, item)| {
                let dict = PyDict::new(py);
                dict.set_item("id", id)?;
                let type_name = match item {
                    RegistryItem::EventLog(_) => "EventLog",
                    RegistryItem::OCEL(_) => "OCEL",
                    RegistryItem::IndexLinkedOCEL(_) => "IndexLinkedOCEL",
                    RegistryItem::SlimLinkedOCEL(_) => "SlimLinkedOCEL",
                    RegistryItem::EventLogActivityProjection(_) => "EventLogActivityProjection",
                };
                dict.set_item("type", type_name)?;
                Ok(dict.into())
            })
            .collect()
    })
}

/// Remove an item from the registry
///
/// # Arguments
/// * `item_id` - ID of the item to remove
///
/// # Returns
/// True if the item was removed, False if it didn't exist
#[pyfunction]
fn remove_item(item_id: String, py: Python<'_>) -> PyResult<bool> {
    let state = get_or_create_app_state(py)?;
    let mut state_guard = state.items.write().unwrap();
    Ok(state_guard.remove(&item_id).is_some())
}

/// Add a registry item from DataFrame(s)
///
/// # Arguments
/// * `item_type` - Type of item to create ("EventLog" or "OCEL")
/// * `data` - For EventLog: single DataFrame; For OCEL: dict with 'events', 'objects', 'relations', 'o2o' DataFrames
/// * `item_id` - Optional ID to assign (auto-generated if not provided)
///
/// # Returns
/// The ID of the created item in the registry
#[pyfunction]
#[pyo3(signature = (item_type, data, item_id=None))]
fn import_item_from_df(
    item_type: String,
    data: &Bound<'_, PyAny>,
    item_id: Option<String>,
    py: Python<'_>,
) -> PyResult<String> {
    let state = get_or_create_app_state(py)?;
    let id = item_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let item = match item_type.as_str() {
        "EventLog" => {
            // Expect a single DataFrame
            let py_df: PyDataFrame = data.extract()?;
            let df: polars::frame::DataFrame = py_df.into();
            let log = convert_dataframe_to_log(&df).map_err(|e| {
                PyTypeError::new_err(format!("Failed to convert DataFrame to EventLog: {:?}", e))
            })?;
            RegistryItem::EventLog(log)
        }
        "OCEL" => {
            // Expect a dict with 'events', 'objects', 'relations', 'o2o' DataFrames
            let dict = data.downcast::<PyDict>().map_err(|_| {
                PyTypeError::new_err(
                    "OCEL requires a dict with 'events', 'objects', 'relations', 'o2o' DataFrames",
                )
            })?;

            let get_df = |key: &str| -> PyResult<polars::frame::DataFrame> {
                let py_df: PyDataFrame = dict
                    .get_item(key)?
                    .ok_or_else(|| PyTypeError::new_err(format!("Missing '{}' DataFrame", key)))?
                    .extract()?;
                Ok(py_df.into())
            };
            let ocel_df = OCEL2DataFrames {
                events: get_df("events")?,
                objects: get_df("objects")?,
                object_changes: get_df("object_changes")?,
                o2o: get_df("o2o")?,
                e2o: get_df("relations")?,
            };
            let ocel_df_ref = OCEL2DataFramesRef {
                events: &ocel_df.events,
                objects: &ocel_df.objects,
                object_changes: &ocel_df.object_changes,
                o2o: &ocel_df.o2o,
                e2o: &ocel_df.e2o,
            };

            let ocel = crate::ocel::df_to_ocel(ocel_df_ref).map_err(|e| {
                PyTypeError::new_err(format!("Failed to convert DataFrames to OCEL: {:?}", e))
            })?;
            RegistryItem::OCEL(ocel)
        }
        _ => {
            return Err(PyTypeError::new_err(format!(
                "Cannot create '{}' from DataFrame. Supported types: EventLog, OCEL",
                item_type
            )));
        }
    };

    state.items.write().unwrap().insert(id.clone(), item);
    Ok(id)
}

// Thread-safe AppState management
// Uses OnceLock for lazy initialization and AppState's internal RwLock for concurrent access
use std::sync::OnceLock;

static GLOBAL_APP_STATE: OnceLock<AppState> = OnceLock::new();

fn get_or_create_app_state(_py: Python<'_>) -> PyResult<&'static AppState> {
    Ok(GLOBAL_APP_STATE.get_or_init(AppState::default))
}
