"""Progress display for binding calls and imports."""
from __future__ import annotations

from typing import Any, Callable, Literal, Optional, Tuple, Union

ProgressCallback = Callable[[str, int, Optional[int]], None]
"""Called as `(stage, done, total)`. `total` is None when the size of the stage is unknown."""

ProgressSetting = Union[Literal["tqdm"], ProgressCallback, None]
"""The global setting: tqdm bars, a callback, or None for no progress display."""

ProgressOption = Union[Literal["tqdm", False], ProgressCallback, None]
"""The per-call option: like `ProgressSetting`, with False for none and None for the global
setting."""

_setting: ProgressSetting = None


def set_progress(value: ProgressSetting) -> None:
    """Show the progress of every binding call and import (`import_item`, `r4pm.df.import_ocel`).

    Args:
        value: "tqdm" for a progress bar per stage (requires tqdm, `pip install r4pm[progress]`),
            a callable `(stage, done, total)`, or None (the default) for no progress display.
            A call's `progress=` argument overrides this for one call.
    """
    global _setting
    _check(value)
    if value == "tqdm":
        _import_tqdm()
    _setting = None if value is False else value


def get_progress() -> ProgressSetting:
    """The current global progress setting (see `set_progress`)."""
    return _setting


def _check(value: Any) -> None:
    if not (value is None or value is False or value == "tqdm" or callable(value)):
        raise ValueError(f'progress must be "tqdm", a callable, False or None, not {value!r}')


def resolve(option: ProgressOption) -> Tuple[Optional[ProgressCallback], Callable[[], None]]:
    """The callback for one call, and a function to call once the call ended."""
    _check(option)
    value = _setting if option is None else option
    if value is None or value is False:
        return None, _nothing
    if isinstance(value, str):
        bars = _TqdmBars()
        return bars, bars.close
    return value, _nothing


def _nothing() -> None:
    pass


def _import_tqdm() -> Any:
    try:
        from tqdm.auto import tqdm  # type: ignore[import]
    except ImportError as e:
        raise ImportError(
            'progress="tqdm" requires tqdm. Install with: pip install r4pm[progress]'
        ) from e
    return tqdm


class _TqdmBars:
    """One tqdm bar per stage, replaced when the stage changes."""

    def __init__(self) -> None:
        self._tqdm = _import_tqdm()
        self._bar: Any = None
        self._stage: Optional[str] = None

    def __call__(self, stage: str, done: int, total: Optional[int]) -> None:
        if self._bar is None or stage != self._stage:
            self.close()
            self._stage = stage
            self._bar = self._tqdm(desc=stage, total=total)
        elif total != self._bar.total:
            self._bar.total = total
        self._bar.update(done - self._bar.n)

    def close(self) -> None:
        if self._bar is not None:
            self._bar.close()
            self._bar = None
