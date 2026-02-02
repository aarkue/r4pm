from typing import Any, Dict, Optional
import polars as pl

# ============================================================================
# XES Import/Export
# ============================================================================

def import_xes(
    path: str,
    date_format: Optional[str] = None,
    print_debug: Optional[bool] = None
) -> tuple[pl.DataFrame, str]:
    """
    Import XES event log.
    
    Args:
        path: Path to .xes or .xes.gz file
        date_format: Optional date format for parsing (see chrono strftime)
        print_debug: Enable debug output
    
    Returns:
        Tuple of (DataFrame with events, JSON string with log metadata)
    """
    ...

def export_xes(df: pl.DataFrame, path: str) -> None:
    """
    Export DataFrame as XES file.
    
    Args:
        df: Polars DataFrame with event data
        path: Output path (.xes or .xes.gz)
    """
    ...

# ============================================================================
# OCEL Import
# ============================================================================

def rs_ocel_to_pm4py(ocel_rs: Dict[str, pl.DataFrame]) -> Any:
    """
    Convert Polars OCEL DataFrames to PM4PY OCEL object.
    
    Args:
        ocel_rs: Dict of DataFrames from import_ocel_xml/json
    
    Returns:
        PM4PY OCEL object
    """
    ...

def import_ocel_xml(path: str) -> Dict[str, pl.DataFrame]:
    """
    Import OCEL2 from XML.
    
    Returns:
        Dict with DataFrames: 'events', 'objects', 'relations', 'o2o', 'object_changes'
    """
    ...

def import_ocel_xml_pm4py(path: str) -> Any:
    """
    Import OCEL2 XML and convert to PM4PY format.
    """
    ...

def import_ocel_json(path: str) -> Dict[str, pl.DataFrame]:
    """
    Import OCEL2 from JSON.
    
    Returns:
        Dict with DataFrames: 'events', 'objects', 'relations', 'o2o', 'object_changes'
    """
    ...

def import_ocel_json_pm4py(path: str) -> Any:
    """
    Import OCEL2 JSON and convert to PM4PY format.
    """
    ...

def import_ocel(path: str) -> dict[str, pl.DataFrame]:
    """
    Import OCEL2 from File.
    
    Returns:
        Dict with DataFrames: 'events', 'objects', 'relations', 'o2o', 'object_changes'
    """
    ...

def import_ocel_pm4py(path: str):
    """Import OCEL2 JSON and convert to PM4PY format."""
    ...


def export_ocel(ocel: dict[str, pl.DataFrame], path: str):
    """
    Export Polars OCEL DataFrames to File
    
    Args:
        ocel: Dict of DataFrames from import_ocel_xml/json
        path: Output path (e.g., .ocel.xml or .ocel.json)
    
    """
    ...

def export_ocel_pm4py(ocel, path: str):
    """
    Export PM4Py OCEL to File
    
    Args:
        ocel: PM4Py OCEL object
        path: Output path (e.g., .ocel.xml or .ocel.json)
    
    """
    ...
