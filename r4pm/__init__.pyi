"""r4pm: Rust for Process Mining (Python Version)

"""
from typing import Any, Dict, List, Optional, Union
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


from . import df as df

__all__: List[str]
__version__: str
