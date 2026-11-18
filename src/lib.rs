use std::ffi::CString;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use process_mining::bindings::{
    call_resolved_with_outcome, get_fn_binding, list_functions_meta, AppState, Binding, CallError,
    CallOutcome, RegistryItem, RegistryItemKind,
};
use process_mining::core::event_data::case_centric::constants::ACTIVITY_NAME;
use process_mining::core::event_data::case_centric::{
    dataframe::{convert_dataframe_to_log, convert_log_to_dataframe},
    xes::{stream_xes, XESImportOptions},
    AttributeValue, XESEditableAttribute,
};
use process_mining::core::event_data::object_centric::linked_ocel::LinkedOCELAccess;
use process_mining::core::process_models::case_centric::petri_net::pnml::{
    export_pnml, import_pnml,
};
use process_mining::core::process_models::case_centric::petri_net::Marking;
use process_mining::utils::control::{Control, Progress};
use process_mining::EventLog;
use process_mining::Exportable;
use process_mining::PetriNet;
use pyo3::{
    exceptions::{PyException, PyKeyboardInterrupt, PyTypeError, PyUserWarning, PyValueError},
    prelude::*,
    types::{PyDict, PyList},
};
use pyo3_polars::PyDataFrame;
use serde_json::Value as JsonValue;

// Subclasses `TypeError`, which binding calls raised before, so existing handlers keep working
pyo3::create_exception!(
    r4pm,
    BindingError,
    PyTypeError,
    "A binding call failed in the Rust library."
);

use crate::ocel::{
    export_ocel_rs, import_ocel_json_rs, import_ocel_rs, import_ocel_xml_rs, OCEL2DataFrames,
    OCEL2DataFramesRef,
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
#[pyo3(signature = (path, date_format=None, print_debug=None))]
fn import_xes_rs(
    py: Python<'_>,
    path: String,
    date_format: Option<String>,
    print_debug: Option<bool>,
) -> PyResult<(PyDataFrame, String)> {
    let debug = print_debug.unwrap_or_default();
    let (df, outer) = py.detach(|| -> PyResult<_> {
        if debug {
            println!("Starting XES Import");
        }
        let start_now = Instant::now();
        let mut now = Instant::now();
        let file = File::open(&path).map_err(|e| PyException::new_err(e.to_string()))?;
        let (mut stream, outer_data) = stream_xes(
            file,
            xes_format(&path),
            XESImportOptions {
                date_format,
                ..Default::default()
            },
        )
        .map_err(|e| PyException::new_err(e.to_string()))?;
        let traces = stream.collect();
        if debug {
            println!("Importing XES Log took {:.2?}", now.elapsed());
        }
        let outer_log_data_string = serde_json::to_string(&outer_data).map_err(|e| {
            PyTypeError::new_err(format!("Failed to convert outer log data to JSON: {e:?}"))
        })?;
        let log = EventLog::from_traces_and_log_data(traces, outer_data);
        now = Instant::now();
        let converted_log = convert_log_to_dataframe(&log, debug)
            .map_err(|e| PyException::new_err(format!("Failed to convert log: {e}")))?;
        if debug {
            println!("Finished Converting Log; Took {:.2?}", now.elapsed());
            println!("Total duration: {:.2?}", start_now.elapsed());
        }
        Ok((converted_log, outer_log_data_string))
    })?;
    Ok((PyDataFrame(df), outer))
}

fn xes_format(path: &str) -> &'static str {
    if path.to_ascii_lowercase().ends_with(".gz") {
        "xes.gz"
    } else {
        "xes"
    }
}

#[pyfunction]
fn export_xes_rs(py: Python<'_>, df: PyDataFrame, path: String) -> PyResult<()> {
    let df: polars::frame::DataFrame = df.into();
    py.detach(|| {
        let log = convert_dataframe_to_log(&df).map_err(|e| {
            PyTypeError::new_err(format!("Failed to convert dataframe to log: {e:?}"))
        })?;
        log.export_to_path_as(&path, xes_format(&path), Default::default())
            .map_err(|e| PyTypeError::new_err(format!("Failed to export XES: {e:?}")))
    })
}

/// Import a Petri net from a PNML file
///
/// Returns a JSON encoding of the [PetriNet], with `initial_marking` and `final_marking` set to
/// `null` when the file has none.
///
/// * `path` - The filepath of the .pnml file to import
#[pyfunction]
fn import_pnml_rs(py: Python<'_>, path: String) -> PyResult<String> {
    py.detach(|| {
        let import = || {
            let file = File::open(&path).map_err(|e| e.to_string())?;
            import_pnml(BufReader::new(file)).map_err(|e| e.to_string())
        };
        let (net, initial, fin) =
            import().map_err(|e| PyException::new_err(format!("Failed to import PNML: {e}")))?;
        let mut value = serde_json::to_value(&net)
            .map_err(|e| PyTypeError::new_err(format!("Failed to serialize PetriNet: {e}")))?;
        let to_json = |m: Option<Marking>| {
            m.map(|m| serde_json::to_value(m).expect("a marking serializes to JSON"))
                .unwrap_or(JsonValue::Null)
        };
        value["initial_marking"] = to_json(initial);
        value["final_marking"] = to_json(fin);
        Ok(value.to_string())
    })
}

/// Export a Petri net to a PNML file
///
/// * `net_json` - A JSON encoding of the [PetriNet] (as returned by `import_pnml_rs`), with
///   optional `initial_marking` and `final_marking`
/// * `path` - The filepath of the .pnml file to write
#[pyfunction]
fn export_pnml_rs(py: Python<'_>, net_json: String, path: String) -> PyResult<()> {
    py.detach(|| {
        let invalid =
            |e: serde_json::Error| PyValueError::new_err(format!("Invalid PetriNet JSON: {e}"));
        let value: JsonValue = serde_json::from_str(&net_json).map_err(invalid)?;
        let net: PetriNet = serde_json::from_value(value.clone()).map_err(invalid)?;
        let marking = |key: &str| -> PyResult<Option<Marking>> {
            match value.get(key) {
                None | Some(JsonValue::Null) => Ok(None),
                Some(m) => serde_json::from_value(m.clone()).map(Some).map_err(invalid),
            }
        };
        let initial = marking("initial_marking")?;
        let fin = marking("final_marking")?;
        let export = || {
            let file = File::create(&path).map_err(|e| e.to_string())?;
            export_pnml(BufWriter::new(file), &net, initial.as_ref(), fin.as_ref())
                .map_err(|e| e.to_string())
        };
        export().map_err(|e| PyException::new_err(format!("Failed to export PNML: {e}")))
    })
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
    m.add_function(wrap_pyfunction!(import_pnml_rs, m)?)?;
    m.add_function(wrap_pyfunction!(export_pnml_rs, m)?)?;

    m.add_function(wrap_pyfunction!(list_bindings, m)?)?;
    m.add_function(wrap_pyfunction!(call_binding, m)?)?;
    m.add_function(wrap_pyfunction!(import_item, m)?)?;
    m.add_function(wrap_pyfunction!(import_item_from_json, m)?)?;
    m.add_function(wrap_pyfunction!(convert_item, m)?)?;
    m.add_function(wrap_pyfunction!(export_item, m)?)?;
    m.add_function(wrap_pyfunction!(item_to_df, m)?)?;
    m.add_function(wrap_pyfunction!(item_to_json, m)?)?;
    m.add_function(wrap_pyfunction!(item_kind, m)?)?;
    m.add_function(wrap_pyfunction!(list_items, m)?)?;
    m.add_function(wrap_pyfunction!(list_item_kinds, m)?)?;
    m.add_function(wrap_pyfunction!(remove_item, m)?)?;
    m.add_function(wrap_pyfunction!(import_item_from_df, m)?)?;
    m.add_function(wrap_pyfunction!(event_log_traces, m)?)?;
    m.add("BindingError", m.py().get_type::<BindingError>())?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("__variant__", VARIANT)?;
    m.add("__features__", ENABLED_FEATURES)?;
    m.add_function(wrap_pyfunction!(has_feature, m)?)?;

    Ok(())
}

// ENABLED_FEATURES, generated by build.rs
include!(concat!(env!("OUT_DIR"), "/enabled_features.rs"));

const VARIANT: &str = if cfg!(feature = "full") { "full" } else { "default" };

/// Check whether an optional feature is compiled into this build
///
/// * `name` - Feature name, e.g. `"ocel-duckdb"`
#[pyfunction]
fn has_feature(name: &str) -> bool {
    ENABLED_FEATURES.contains(&name)
}

/// List all available binding functions
///
/// Returns a list of dictionaries containing metadata about each function
/// Each dictionary includes: name, description, arguments, and return type
#[pyfunction]
fn list_bindings(py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
    list_functions_meta()
        .iter()
        .map(|f| {
            let dict = PyDict::new(py);
            dict.set_item("id", &f.id)?;
            dict.set_item("name", &f.name)?;
            dict.set_item("description", &f.docs)?;
            dict.set_item("module", &f.module)?;

            let args = PyList::empty(py);
            for (arg_name, arg_schema) in &f.args {
                let arg_dict = PyDict::new(py);
                arg_dict.set_item("name", arg_name)?;
                arg_dict.set_item("schema", arg_schema.to_string())?;
                args.append(arg_dict)?;
            }
            dict.set_item("arguments", args)?;
            dict.set_item("required_arguments", PyList::new(py, f.required_args.iter())?)?;
            dict.set_item("return_schema", f.return_type.to_string())?;
            Ok(dict.into_any().unbind())
        })
        .collect()
}

/// Call a binding function by ID with provided arguments
///
/// Registry arguments may be stored IDs, file paths, `{"path": ...}`, `{"bytes": ..., "format":
/// ...}` or inline JSON. Items loaded only for the call are removed again afterwards.
///
/// The binding runs through [`run_controlled`]: its progress and warnings reach Python, and
/// Ctrl+C cancels it.
///
/// # Arguments
/// * `function_id` - ID of the function to call (e.g., "process_mining::num_events")
/// * `args_json` - JSON string containing the function arguments
/// * `num_threads` - Optional number of worker threads for parallel bindings (default: all cores)
/// * `progress` - Optional callable `(stage, done, total)` receiving progress reports
///
/// # Returns
/// JSON string containing the result (either a value or a registry item ID)
#[pyfunction]
#[pyo3(signature = (function_id, args_json, num_threads=None, progress=None))]
fn call_binding(
    py: Python<'_>,
    function_id: String,
    args_json: String,
    num_threads: Option<usize>,
    progress: Option<Bound<'_, PyAny>>,
) -> PyResult<String> {
    let state = app_state();
    let args: JsonValue = serde_json::from_str(&args_json)
        .map_err(|e| PyTypeError::new_err(format!("Invalid JSON arguments: {}", e)))?;
    let binding = get_fn_binding(&function_id)
        .ok_or_else(|| PyTypeError::new_err(format!("Function '{}' not found", function_id)))?;

    let (failure, outcome) = run_controlled(py, progress.as_ref(), |ctl| {
        run_binding(binding, &args, state, ctl, num_threads)
    })?;

    match (failure, outcome) {
        (Some(err), Ok(outcome)) => {
            // Finished before it noticed the cancel. Nobody would own an item under a generated id.
            if args.get("output_id").is_none_or(JsonValue::is_null) {
                for id in &outcome.minted {
                    state.remove(id);
                }
            }
            Err(err)
        }
        (Some(err), Err(_)) => Err(err),
        (None, Err(CallError::Cancelled)) => Err(PyKeyboardInterrupt::new_err("cancelled")),
        (None, Err(CallError::Failed(e))) => Err(BindingError::new_err(format!(
            "Function call failed: {}",
            e
        ))),
        (None, Ok(outcome)) => String::from_utf8(outcome.result)
            .map_err(|e| PyTypeError::new_err(format!("Result was not valid UTF-8: {}", e))),
    }
}

const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Run `work` on its own thread under a [`Control`], while this thread delivers its progress
/// (to `progress`) and warnings and checks for Ctrl+C. Python only handles signals on the main
/// thread, so this cannot be done from the (possibly parallel) progress reports. Ctrl+C cancels
/// the work, which stops at its next cancellation check.
///
/// Returns the first Python exception raised while delivering (e.g., `KeyboardInterrupt`, also
/// from the callback), which cancelled the work, next to the work's outcome. The work may have
/// finished before it noticed the cancel; the caller then discards its result. A panic becomes
/// [`CallError::Failed`].
fn run_controlled<T: Send>(
    py: Python<'_>,
    progress: Option<&Bound<'_, PyAny>>,
    work: impl FnOnce(&Control<'_>) -> Result<T, CallError> + Send,
) -> PyResult<(Option<PyErr>, Result<T, CallError>)> {
    let cancel = AtomicBool::new(false);
    let reports = Mutex::new(Reports::default());
    let wake = Condvar::new();
    let mut ctl = Control::new()
        .cancel_flag(&cancel)
        .on_warning(|w| lock(&reports).warnings.push(w.to_string()));
    // Only with a listener, as reporting can cost extra work (e.g., counting rows in advance)
    if progress.is_some() {
        ctl = ctl.on_progress(|p| lock(&reports).push_progress(p));
    }

    let mut failure: Option<PyErr> = None;
    let outcome = std::thread::scope(|s| {
        let worker = std::thread::Builder::new()
            .name("r4pm-worker".to_string())
            // As large as a typical main thread stack, which calls ran on before
            .stack_size(8 << 20)
            .spawn_scoped(s, || {
                let outcome = work(&ctl);
                lock(&reports).finished = true;
                wake.notify_all();
                outcome
            })
            .map_err(|e| BindingError::new_err(format!("Failed to start the call: {e}")))?;
        loop {
            let finished = py.detach(|| {
                let reports = lock(&reports);
                wake.wait_timeout_while(reports, POLL_INTERVAL, |r| !r.finished)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
                    .finished
            }) || worker.is_finished();
            deliver(py, &reports, progress, &cancel, &mut failure, !finished);
            if finished {
                break;
            }
        }
        PyResult::Ok(
            worker
                .join()
                .unwrap_or_else(|_| Err(CallError::Failed("the call panicked".to_string()))),
        )
    })?;
    Ok((failure, outcome))
}

/// What a running binding reported and not yet delivered to Python
#[derive(Default)]
struct Reports {
    progress: Vec<Progress>,
    /// The stage of the last progress report, and whether it was the first of its stage
    last: Option<(std::borrow::Cow<'static, str>, bool)>,
    warnings: Vec<String>,
    finished: bool,
}

impl Reports {
    /// Keeps the first and the latest report of each stage
    fn push_progress(&mut self, p: Progress) {
        let same_stage = self
            .last
            .as_ref()
            .is_some_and(|(stage, _)| *stage == p.stage);
        let last_was_first = self.last.as_ref().is_some_and(|(_, first)| *first);
        if same_stage && !last_was_first {
            if let Some(last) = self.progress.last_mut() {
                *last = p;
                return;
            }
        }
        self.last = Some((p.stage.clone(), !same_stage));
        self.progress.push(p);
    }
}

fn lock(reports: &Mutex<Reports>) -> MutexGuard<'_, Reports> {
    reports.lock().unwrap_or_else(PoisonError::into_inner)
}

fn run_binding(
    binding: &Binding,
    args: &JsonValue,
    state: &AppState,
    ctl: &Control<'_>,
    num_threads: Option<usize>,
) -> Result<CallOutcome, CallError> {
    match num_threads {
        None => call_resolved_with_outcome(binding, args, state, ctl),
        Some(n) => rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build()
            .map_err(|e| CallError::Failed(format!("Failed to create a thread pool: {e}")))
            .and_then(|pool| {
                pool.install(|| call_resolved_with_outcome(binding, args, state, ctl))
            }),
    }
}

/// Hand pending warnings and progress to Python and check for Ctrl+C. The first exception
/// (e.g., `KeyboardInterrupt`, also from the callback) cancels the call and is kept in `failure`;
/// after it, nothing more is delivered.
fn deliver(
    py: Python<'_>,
    reports: &Mutex<Reports>,
    callback: Option<&Bound<'_, PyAny>>,
    cancel: &AtomicBool,
    failure: &mut Option<PyErr>,
    check_signals: bool,
) {
    let (progress, warnings) = {
        let mut r = lock(reports);
        (
            std::mem::take(&mut r.progress),
            std::mem::take(&mut r.warnings),
        )
    };
    if failure.is_some() {
        return;
    }
    let delivered = (|| -> PyResult<()> {
        for w in warnings {
            let msg = CString::new(w.replace('\0', "")).expect("NUL bytes were removed");
            PyErr::warn(py, py.get_type::<PyUserWarning>().as_any(), &msg, 1)?;
        }
        if let Some(callback) = callback {
            for p in progress {
                callback.call1((p.stage.as_ref(), p.done, p.total))?;
            }
        }
        if check_signals {
            py.check_signals()?;
        }
        Ok(())
    })();
    if let Err(e) = delivered {
        cancel.store(true, Ordering::Relaxed);
        *failure = Some(e);
    }
}

fn parse_kind(name: &str) -> PyResult<RegistryItemKind> {
    name.parse()
        .map_err(|e| PyTypeError::new_err(format!("Invalid item type '{}': {}", name, e)))
}

fn not_found(item_id: &str) -> PyErr {
    PyTypeError::new_err(format!("Item not found: {}", item_id))
}

fn insert_item(item: RegistryItem, item_id: Option<String>) -> String {
    let id = item_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    app_state().add(id.clone(), item);
    id
}

/// Load a registry item from a file path
///
/// The import runs through [`run_controlled`], like a binding call: its progress (for most file
/// formats, stage `"Importing"` with the bytes read) and warnings reach Python, and Ctrl+C
/// cancels it. A cancelled import leaves the registry unchanged.
///
/// # Arguments
/// * `item_type` - Type of item to load (e.g., "EventLog", "OCEL", "SlimLinkedOCEL", "AcceptingPetriNet")
/// * `file_path` - Path to the file to load
/// * `item_id` - Optional ID to assign to the loaded item (auto-generated if not provided)
/// * `progress` - Optional callable `(stage, done, total)` receiving progress reports
///
/// # Returns
/// The ID of the loaded item in the registry
#[pyfunction]
#[pyo3(signature = (item_type, file_path, item_id=None, progress=None))]
fn import_item(
    py: Python<'_>,
    item_type: String,
    file_path: String,
    item_id: Option<String>,
    progress: Option<Bound<'_, PyAny>>,
) -> PyResult<String> {
    let kind = parse_kind(&item_type)?;
    let (failure, outcome) = run_controlled(py, progress.as_ref(), |ctl| {
        RegistryItem::load_from_path_with(&kind, &file_path, ctl)
    })?;
    match (failure, outcome) {
        (Some(err), _) => Err(err),
        (None, Err(CallError::Cancelled)) => Err(PyKeyboardInterrupt::new_err("cancelled")),
        (None, Err(CallError::Failed(e))) => Err(PyTypeError::new_err(format!(
            "Failed to load {} from {}: {}",
            item_type, file_path, e
        ))),
        (None, Ok(item)) => Ok(insert_item(item, item_id)),
    }
}

/// Add a registry item from its JSON form (as returned by `item_to_json`)
#[pyfunction]
#[pyo3(signature = (item_type, json, item_id=None))]
fn import_item_from_json(
    py: Python<'_>,
    item_type: String,
    json: String,
    item_id: Option<String>,
) -> PyResult<String> {
    let kind = parse_kind(&item_type)?;
    py.detach(|| {
        let value: JsonValue = serde_json::from_str(&json)
            .map_err(|e| PyValueError::new_err(format!("Invalid JSON: {e}")))?;
        let item = RegistryItem::from_json_value(&kind, &value).map_err(|e| {
            PyValueError::new_err(format!("Failed to read {} from JSON: {}", item_type, e))
        })?;
        Ok(insert_item(item, item_id))
    })
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
    py: Python<'_>,
    item_id: String,
    target_type: String,
    new_item_id: Option<String>,
) -> PyResult<String> {
    let target_kind = parse_kind(&target_type)?;
    py.detach(|| {
        let converted = {
            let items = app_state().read();
            let source = items.get(&item_id).ok_or_else(|| not_found(&item_id))?;
            source
                .convert(target_kind)
                .map_err(|e| PyTypeError::new_err(format!("Failed to convert item: {}", e)))?
        };
        Ok(insert_item(converted, new_item_id))
    })
}

/// Export a registry item to a file path
///
/// # Arguments
/// * `item_id` - ID of the item to export
/// * `file_path` - Path where the file should be written
#[pyfunction]
fn export_item(py: Python<'_>, item_id: String, file_path: String) -> PyResult<()> {
    py.detach(|| {
        let items = app_state().read();
        let item = items.get(&item_id).ok_or_else(|| not_found(&item_id))?;
        item.export_to_path(&file_path)
            .map_err(|e| PyTypeError::new_err(format!("Failed to export item: {}", e)))
    })
}

enum ItemFrames {
    Log(polars::frame::DataFrame),
    Ocel(OCEL2DataFrames),
}

/// Get a registry item as a Polars DataFrame
///
/// # Returns
/// For EventLog: DataFrame with event data
/// For OCEL: Dictionary with multiple DataFrames (events, objects, o2o, relations, object_changes)
#[pyfunction]
fn item_to_df(py: Python<'_>, item_id: String) -> PyResult<Py<PyAny>> {
    let frames = py.detach(|| {
        let items = app_state().read();
        let item = items.get(&item_id).ok_or_else(|| not_found(&item_id))?;
        match item {
            RegistryItem::EventLog(log) => convert_log_to_dataframe(log, false)
                .map(ItemFrames::Log)
                .map_err(|e| PyTypeError::new_err(format!("Failed to convert log: {}", e))),
            RegistryItem::OCEL(ocel) => Ok(ItemFrames::Ocel(crate::ocel::ocel2_to_df(ocel))),
            // TODO: Optimize ocel2_to_df to only take LinkedOCELAccess
            RegistryItem::IndexLinkedOCEL(locel) => Ok(ItemFrames::Ocel(
                crate::ocel::ocel2_to_df(&locel.construct_ocel()),
            )),
            RegistryItem::SlimLinkedOCEL(locel) => Ok(ItemFrames::Ocel(
                crate::ocel::ocel2_to_df(&locel.construct_ocel()),
            )),
            _ => Err(PyTypeError::new_err(format!(
                "A {} cannot be converted to a DataFrame",
                item.kind()
            ))),
        }
    })?;
    match frames {
        ItemFrames::Log(df) => Ok(PyDataFrame(df).into_pyobject(py)?.into_any().unbind()),
        ItemFrames::Ocel(dfs) => Ok(crate::ocel::ocel_dfs_to_py(dfs)
            .into_pyobject(py)?
            .into_any()
            .unbind()),
    }
}

/// Get a registry item in its JSON form
///
/// For large items (event logs, OCELs) this is a full serialization.
#[pyfunction]
fn item_to_json(py: Python<'_>, item_id: String) -> PyResult<String> {
    py.detach(|| {
        let items = app_state().read();
        let item = items.get(&item_id).ok_or_else(|| not_found(&item_id))?;
        item.to_value()
            .map(|v| v.to_string())
            .map_err(|e| PyTypeError::new_err(format!("Failed to serialize item: {}", e)))
    })
}

/// The type name of a registry item, or `None` if no item has this ID
#[pyfunction]
fn item_kind(item_id: String) -> Option<&'static str> {
    app_state().read().get(&item_id).map(|i| i.kind().name())
}

/// List all items currently in the registry
///
/// # Returns
/// List of dictionaries with 'id' and 'type' fields
#[pyfunction]
fn list_items(py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
    let items: Vec<(String, &'static str)> = app_state()
        .read()
        .iter()
        .map(|(id, item)| (id.clone(), item.kind().name()))
        .collect();
    items
        .into_iter()
        .map(|(id, kind)| {
            let dict = PyDict::new(py);
            dict.set_item("id", id)?;
            dict.set_item("type", kind)?;
            Ok(dict.into_any().unbind())
        })
        .collect()
}

/// List the names of all registry item types
#[pyfunction]
fn list_item_kinds() -> Vec<&'static str> {
    RegistryItemKind::all_registered_kinds()
        .iter()
        .map(|k| k.name())
        .collect()
}

/// Remove an item from the registry
///
/// # Returns
/// True if the item was removed, False if it didn't exist
#[pyfunction]
fn remove_item(item_id: String) -> bool {
    app_state().remove(&item_id).is_some()
}

/// The activity sequence of every trace of an event log item, in log order
///
/// For an `EventLogActivityProjection`, every variant is repeated by its frequency.
///
/// * `item_id` - ID of an `EventLog` or `EventLogActivityProjection`
#[pyfunction]
fn event_log_traces(py: Python<'_>, item_id: String) -> PyResult<Vec<Vec<String>>> {
    py.detach(|| {
        let items = app_state().read();
        match items.get(&item_id).ok_or_else(|| not_found(&item_id))? {
            // Same activity fallback as the activity projection, so traces line up with its variants
            RegistryItem::EventLog(log) => Ok(log
                .traces
                .iter()
                .map(|t| {
                    t.events
                        .iter()
                        .map(|e| match e.attributes.get_by_key(ACTIVITY_NAME) {
                            Some(attr) => match &attr.value {
                                AttributeValue::String(s) => s.clone(),
                                _ => "No Activity".to_string(),
                            },
                            None => "No Activity".to_string(),
                        })
                        .collect()
                })
                .collect()),
            RegistryItem::EventLogActivityProjection(proj) => Ok(proj
                .traces
                .iter()
                .flat_map(|(acts, freq)| {
                    let trace = proj.reconstruct_activities(acts);
                    std::iter::repeat_n(trace, *freq as usize)
                })
                .collect()),
            other => Err(PyTypeError::new_err(format!(
                "Expected an EventLog or EventLogActivityProjection, found {}",
                other.kind()
            ))),
        }
    })
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
    py: Python<'_>,
    item_type: String,
    data: &Bound<'_, PyAny>,
    item_id: Option<String>,
) -> PyResult<String> {
    let item = match item_type.as_str() {
        "EventLog" => {
            let df: polars::frame::DataFrame = data.extract::<PyDataFrame>()?.into();
            let log = py.detach(|| convert_dataframe_to_log(&df)).map_err(|e| {
                PyTypeError::new_err(format!("Failed to convert DataFrame to EventLog: {:?}", e))
            })?;
            RegistryItem::EventLog(log)
        }
        "OCEL" => {
            let dict = data.cast::<PyDict>().map_err(|_| {
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
            let ocel = py
                .detach(|| {
                    crate::ocel::df_to_ocel(OCEL2DataFramesRef {
                        events: &ocel_df.events,
                        objects: &ocel_df.objects,
                        object_changes: &ocel_df.object_changes,
                        o2o: &ocel_df.o2o,
                        e2o: &ocel_df.e2o,
                    })
                })
                .map_err(|e| {
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
    Ok(insert_item(item, item_id))
}

static GLOBAL_APP_STATE: OnceLock<AppState> = OnceLock::new();

fn app_state() -> &'static AppState {
    GLOBAL_APP_STATE.get_or_init(AppState::default)
}
