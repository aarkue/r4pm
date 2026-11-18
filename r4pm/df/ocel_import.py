import polars as pl

from .. import _progress
from .._progress import ProgressOption
from ..r4pm import import_ocel_rs

def rs_ocel_to_pm4py(ocel_rs: dict[str, pl.DataFrame]):
    """
    Convert Polars OCEL DataFrames to PM4PY OCEL object.
    
    Args:
        ocel_rs: Dict of DataFrames from import_ocel_xml/json
    
    Returns:
        PM4PY OCEL object
    """
    import pm4py
    ocel_pm4py = pm4py.ocel.OCEL(
        events=ocel_rs["events"].to_pandas().convert_dtypes(),
        objects=ocel_rs["objects"].to_pandas().convert_dtypes(),
        relations=ocel_rs["relations"].to_pandas().convert_dtypes(),
        object_changes=ocel_rs["object_changes"].to_pandas().convert_dtypes(),
        globals={},
        o2o=ocel_rs["o2o"].to_pandas().convert_dtypes(),
        parameters={
            pm4py.objects.ocel.obj.Parameters.EVENT_ID: "ocel:eid",
            pm4py.objects.ocel.obj.Parameters.EVENT_ACTIVITY: "ocel:activity",
            pm4py.objects.ocel.obj.Parameters.EVENT_TIMESTAMP: "ocel:timestamp",
            pm4py.objects.ocel.obj.Parameters.OBJECT_ID: "ocel:oid",
            pm4py.objects.ocel.obj.Parameters.OBJECT_TYPE: "ocel:type",
            pm4py.objects.ocel.obj.Parameters.QUALIFIER: "ocel:qualifier",
            pm4py.objects.ocel.obj.Parameters.CHANGED_FIELD: "ocel:field",
        },
    )
    return ocel_pm4py

def import_ocel_xml(path: str, *, progress: ProgressOption = None) -> dict[str, pl.DataFrame]:
    """
    Import OCEL2 from XML.

    Args:
        progress: See `import_ocel`.

    Returns:
        Dict with DataFrames: 'events', 'objects', 'relations', 'o2o', 'object_changes'
    """
    return import_ocel(path, progress=progress)

def import_ocel_xml_pm4py(path: str, *, progress: ProgressOption = None):
    """Import OCEL2 XML and convert to PM4PY format."""
    return rs_ocel_to_pm4py(import_ocel_xml(path, progress=progress))

def import_ocel_json(path: str, *, progress: ProgressOption = None) -> dict[str, pl.DataFrame]:
    """
    Import OCEL2 from JSON.

    Args:
        progress: See `import_ocel`.

    Returns:
        Dict with DataFrames: 'events', 'objects', 'relations', 'o2o', 'object_changes'
    """
    return import_ocel(path, progress=progress)

def import_ocel_json_pm4py(path: str, *, progress: ProgressOption = None):
    """Import OCEL2 JSON and convert to PM4PY format."""
    return rs_ocel_to_pm4py(import_ocel_json(path, progress=progress))

def import_ocel(path: str, *, progress: ProgressOption = None) -> dict[str, pl.DataFrame]:
    """
    Import OCEL2 from File.

    Reports its progress (for XML and JSON, stage "Importing" with the bytes read). Ctrl+C (or an
    exception from the progress callback) cancels the import; importer warnings are issued as
    Python warnings.

    Args:
        progress: Progress display for this import; see `r4pm.set_progress`. None uses the
            global setting, False shows none.

    Returns:
        Dict with DataFrames: 'events', 'objects', 'relations', 'o2o', 'object_changes'
    """
    callback, done = _progress.resolve(progress)
    try:
        return import_ocel_rs(path, callback)
    finally:
        done()

def import_ocel_pm4py(path: str, *, progress: ProgressOption = None):
    """Import OCEL2 JSON and convert to PM4PY format."""
    return rs_ocel_to_pm4py(import_ocel(path, progress=progress))
