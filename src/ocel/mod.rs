use polars::frame::DataFrame;
use process_mining::{core::io::Importable, Exportable, OCEL};
use process_mining::bindings::CallError;
use pyo3::{
    exceptions::{PyIOError, PyKeyboardInterrupt, PyTypeError},
    pyfunction, Bound, PyAny, PyResult, Python,
};
use pyo3_polars::PyDataFrame;
use std::collections::HashMap;

pub mod to_dataframe;
pub use to_dataframe::{ocel2_to_df, ocel_dfs_to_py};
pub mod from_dataframe;
pub use from_dataframe::df_to_ocel;

pub const OCEL_EVENT_ID_KEY: &str = "ocel:eid";
pub const OCEL_EVENT_TYPE_KEY: &str = "ocel:activity";
pub const OCEL_EVENT_TIMESTAMP_KEY: &str = "ocel:timestamp";
pub const OCEL_OBJECT_ID_KEY: &str = "ocel:oid";
pub const OCEL_OBJECT_ID_2_KEY: &str = "ocel:oid_2";
pub const OCEL_OBJECT_TYPE_KEY: &str = "ocel:type";
pub const OCEL_QUALIFIER_KEY: &str = "ocel:qualifier";
pub const OCEL_CHANGED_FIELD_KEY: &str = "ocel:field";
pub struct OCEL2DataFrames {
    pub objects: DataFrame,
    pub events: DataFrame,
    pub object_changes: DataFrame,
    pub o2o: DataFrame,
    pub e2o: DataFrame,
}

pub struct OCEL2DataFramesRef<'a> {
    pub objects: &'a DataFrame,
    pub events: &'a DataFrame,
    pub object_changes: &'a DataFrame,
    pub o2o: &'a DataFrame,
    pub e2o: &'a DataFrame,
}

/// Import an OCEL 2.0 file as DataFrames
///
/// Runs through [`crate::run_controlled`] like `import_item`: progress, warnings and Ctrl+C.
#[pyfunction]
#[pyo3(signature = (path, progress=None))]
pub fn import_ocel_rs(
    py: Python<'_>,
    path: String,
    progress: Option<Bound<'_, PyAny>>,
) -> PyResult<HashMap<String, PyDataFrame>> {
    let (failure, outcome) = crate::run_controlled(py, progress.as_ref(), |ctl| {
        let ocel = OCEL::import_from_path_with(&path, Default::default(), ctl).map_err(|e| {
            if ctl.is_cancelled() {
                CallError::Cancelled
            } else {
                CallError::Failed(e.to_string())
            }
        })?;
        Ok(ocel2_to_df(&ocel))
    })?;
    match (failure, outcome) {
        (Some(err), _) => Err(err),
        (None, Err(CallError::Cancelled)) => Err(PyKeyboardInterrupt::new_err("cancelled")),
        (None, Err(CallError::Failed(e))) => Err(PyIOError::new_err(format!(
            "Failed to import OCEL: {}",
            e
        ))),
        (None, Ok(dfs)) => Ok(ocel_dfs_to_py(dfs)),
    }
}

#[pyfunction]
#[pyo3(signature = (path, progress=None))]
pub fn import_ocel_xml_rs(
    py: Python<'_>,
    path: String,
    progress: Option<Bound<'_, PyAny>>,
) -> PyResult<HashMap<String, PyDataFrame>> {
    import_ocel_rs(py, path, progress)
}

#[pyfunction]
#[pyo3(signature = (path, progress=None))]
pub fn import_ocel_json_rs(
    py: Python<'_>,
    path: String,
    progress: Option<Bound<'_, PyAny>>,
) -> PyResult<HashMap<String, PyDataFrame>> {
    import_ocel_rs(py, path, progress)
}

pub fn ocel_df_to_rs(
    dfs: HashMap<String, PyDataFrame>,
) -> Result<OCEL, polars::prelude::PolarsError> {
    let ocel_dfs = OCEL2DataFramesRef {
        events: &dfs
            .get("events")
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("Missing 'events' DataFrame"))?
            .0,
        objects: &dfs
            .get("objects")
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("Missing 'objects' DataFrame"))?
            .0,
        object_changes: &dfs
            .get("object_changes")
            .ok_or_else(|| {
                pyo3::exceptions::PyKeyError::new_err("Missing 'object_changes' DataFrame")
            })?
            .0,
        o2o: &dfs
            .get("o2o")
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("Missing 'o2o' DataFrame"))?
            .0,
        e2o: &dfs
            .get("relations")
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("Missing 'relations' DataFrame"))?
            .0,
    };
    df_to_ocel(ocel_dfs)
}

#[pyfunction]
pub fn export_ocel_rs(
    py: Python<'_>,
    ocel: HashMap<String, PyDataFrame>,
    path: String,
) -> PyResult<()> {
    py.detach(|| {
        ocel_df_to_rs(ocel)
            .map_err(|e| PyTypeError::new_err(format!("Failed to convert OCEL:{}", e)))
            .and_then(|ocel_rs| {
                ocel_rs
                    .export_to_path(&path)
                    .map_err(|e| PyTypeError::new_err(format!("Failed to export OCEL: {}", e)))
            })
    })
}