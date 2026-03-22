import polars

from ..r4pm import export_xes_rs

def export_xes(df: polars.DataFrame, path: str):
    """
    Export DataFrame as XES file.
    
    Args:
        df: Polars DataFrame with event data
        path: Output path (.xes or .xes.gz)
    """
    return export_xes_rs(df, path)
