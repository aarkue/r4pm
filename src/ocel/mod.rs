use polars::frame::DataFrame;
use process_mining::{core::io::Importable, Exportable, OCEL};
use pyo3::{exceptions::PyTypeError, pyfunction, PyResult};
use pyo3_polars::PyDataFrame;
use std::{collections::HashMap, fmt::format};

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

#[pyfunction]
pub fn import_ocel_rs(path: String) -> PyResult<HashMap<String, PyDataFrame>> {
    let ocel = OCEL::import_from_path(&path).map_err(|e| {
        pyo3::exceptions::PyIOError::new_err(format!("Failed to import OCEL: {}", e))
    })?;
    let ocel_dfs = ocel2_to_df(&ocel);
    Ok(ocel_dfs_to_py(ocel_dfs))
}

#[pyfunction]
pub fn import_ocel_xml_rs(path: String) -> PyResult<HashMap<String, PyDataFrame>> {
    import_ocel_rs(path)
}

#[pyfunction]
pub fn import_ocel_json_rs(path: String) -> PyResult<HashMap<String, PyDataFrame>> {
    import_ocel_rs(path)
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
pub fn export_ocel_rs(ocel: HashMap<String, PyDataFrame>, path: String) -> PyResult<()> {
    ocel_df_to_rs(ocel)
        .map_err(|e| PyTypeError::new_err(format!("Failed to convert OCEL:{}", e)))
        .and_then(|ocel_rs| {
            ocel_rs
                .export_to_path(&path)
                .map_err(|e| PyTypeError::new_err(format!("Failed to export OCEL: {}", e)))
        })?;
    Ok(())
}
