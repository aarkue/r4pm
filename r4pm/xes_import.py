from typing import Optional
import polars

from .r4pm import import_xes_rs

def import_xes(path: str, date_format: Optional[str] = None, print_debug: Optional[bool] = None) -> tuple[polars.DataFrame, str]:
    """
    Import XES event log.
    
    Args:
        path: Path to .xes or .xes.gz file
        date_format: Optional date format for parsing (see chrono strftime)
        print_debug: Enable debug output
    
    Returns:
        Tuple of (DataFrame with events, JSON string with log metadata)
    """
    return import_xes_rs(path, date_format, print_debug)
