import polars

from .r4pm import export_ocel_rs

def export_ocel(ocel: dict[str, polars.DataFrame], path: str):
    """
    Export Polars OCEL DataFrames to File
    
    Args:
        ocel: Dict of DataFrames from import_ocel_xml/json
        path: Output path (e.g., .ocel.xml or .ocel.json)
    
    """
    export_ocel_rs(ocel, path)


def export_ocel_pm4py(ocel, path: str):
    """
    Export PM4Py OCEL to File
    
    Args:
        ocel: PM4Py OCEL object
        path: Output path (e.g., .ocel.xml or .ocel.json)
    
    """
    ocel = {
        "events": polars.from_pandas(ocel.events),
        "objects": polars.from_pandas(ocel.objects),
        "relations": polars.from_pandas(ocel.relations),
        "object_changes": polars.from_pandas(ocel.object_changes),
        "o2o": polars.from_pandas(ocel.o2o),
    }
    export_ocel_rs(ocel, path)