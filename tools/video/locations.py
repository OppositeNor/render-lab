"""Source locations from Lamlet IR, including honest container fallbacks."""
from __future__ import annotations

from typing import Any

Json = dict[str, Any]


def location(node: Json, fallback: Json) -> Json:
    """Keep the complete IR range; containers without an `at` are approximate."""
    if node.get('at') is not None:
        return dict(node['at'])
    for child in node.get('inlines', []) + node.get('shown', []):
        found = location(child, {})
        if found:
            return {**found, 'fallback': 'IR container has no own location; using its first positioned descendant'}
    return {**fallback, 'fallback': 'IR node has no location; using its enclosing paragraph or file'} if fallback else {}


def describe(source: Json) -> str:
    out = str(source.get('file', '<unknown>'))
    for field in ('line', 'column'):
        if source.get(field) is None:
            break
        out += f':{source[field]}'
    return out


class DiagnosticError(RuntimeError):
    def __init__(self, code: str, message: str, source: Json | None = None, *, beat: str | None = None, field: str | None = None):
        self.diagnostic: Json = {'code': code, 'severity': 'error', 'file': None, 'line': None, 'column': None,
                                 'endLine': None, 'endColumn': None, 'beat': beat, 'field': field, 'message': message,
                                 **(source or {})}
        super().__init__(f'{describe(source)}: {message}' if source else message)
