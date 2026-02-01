"""r4pm: Rust for Process Mining (Python Version)

"""
from typing import Any, Dict, List, Optional, Union
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

# ============================================================================
# Registry Functions
# ============================================================================

def import_item(item_type: str, file_path: str, item_id: Optional[str] = None) -> str:
    """
    Load a file into the registry.
    
    Args:
        item_type: Type name (e.g., "OCEL", "EventLog", "IndexLinkedOCEL", "SlimLinkedOCEL")
        file_path: Path to file to load
        item_id: Optional custom ID (auto-generated if not provided)
    
    Returns:
        Registry item ID
    """
    ...

def convert_item(item_id: str, target_type: str, new_item_id: Optional[str] = None) -> str:
    """
    Convert a registry item to another type.
    
    Args:
        item_id: ID of item to convert
        target_type: Target type name (e.g., "IndexLinkedOCEL")
        new_item_id: Optional ID for converted item (auto-generated if not provided)
    
    Returns:
        New registry item ID
    """
    ...

def export_item(item_id: str, file_path: str) -> None:
    """Export a registry item to a file."""
    ...

def item_to_df(item_id: str) -> Union[pl.DataFrame, Dict[str, pl.DataFrame]]:
    """
    Get registry item as DataFrame(s).
    
    Returns:
        Single DataFrame for EventLog, dict of DataFrames for OCEL
    """
    ...

def list_items() -> List[Dict[str, str]]:
    """
    List all items in the registry.
    
    Returns:
        List of dicts with 'id' and 'type' keys
    """
    ...

def remove_item(item_id: str) -> bool:
    """
    Remove item from registry.
    
    Returns:
        True if removed, False if not found
    """
    ...

def import_item_from_df(
    item_type: str,
    data: Union[pl.DataFrame, Dict[str, pl.DataFrame]],
    item_id: Optional[str] = None
) -> str:
    """
    Create a registry item from DataFrame(s).
    
    Args:
        item_type: Type of item to create ("EventLog" or "OCEL")
        data: For EventLog: single DataFrame with event data.
              For OCEL: dict with 'events', 'objects', 'relations', 'o2o' DataFrames
        item_id: Optional custom ID (auto-generated if not provided)
    
    Returns:
        Registry item ID
    """
    ...

# ============================================================================
# Bindings Introspection
# ============================================================================

def list_bindings() -> List[Dict[str, Any]]:
    """
    List all available binding functions.
    
    Returns:
        List of function metadata dicts with keys: id, name, description, module, 
        arguments, required_arguments, return_schema
    """
    ...

def call_binding(function_id: str, args_json: str) -> str:
    """
    Call a binding function by ID.
    
    Args:
        function_id: Function ID (e.g., "process_mining::bindings::num_events")
        args_json: JSON string with function arguments
    
    Returns:
        JSON string with result (value or registry ID)
    """
    ...

# ============================================================================
# Bindings Submodule
# ============================================================================

# Bindings submodule (organized by Rust module structure)
from . import bindings as bindings

__all__: List[str]

__version__: str
