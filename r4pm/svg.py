"""SVG images returned by the drawing bindings (e.g., `r4pm.bindings.petri_net_svg`)."""
from __future__ import annotations

import os
from typing import Union


class Svg(str):
    """An SVG document. As a `str`, it is the SVG markup itself.

    Notebooks (Jupyter, VS Code) display it as an image.
    """

    def save(self, path: Union[str, "os.PathLike[str]"]) -> None:
        """Write the SVG to a file (e.g., "model.svg")."""
        with open(path, "w", encoding="utf-8") as f:
            f.write(self)

    def _repr_svg_(self) -> str:
        return str(self)

    def __repr__(self) -> str:
        return f"<Svg, {len(self)} characters>"
