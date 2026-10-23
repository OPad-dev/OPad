"""Minimal S-expression reader for KiCad files (no KiCad needed)."""

import re

_TOKEN = re.compile(r'\(|\)|"(?:[^"\\]|\\.)*"|[^\s()"]+')


def parse(text):
    stack = [[]]
    for m in _TOKEN.finditer(text):
        t = m.group(0)
        if t == "(":
            stack.append([])
        elif t == ")":
            done = stack.pop()
            stack[-1].append(done)
        elif t.startswith('"'):
            stack[-1].append(Str(t[1:-1].replace('\\"', '"').replace("\\\\", "\\")))
        else:
            stack[-1].append(t)
    return stack[0][0]


class Str(str):
    """A quoted string, so it is never mistaken for a keyword."""


def head(node):
    return node[0] if isinstance(node, list) and node else None


def children(node, name):
    return [c for c in node[1:] if isinstance(c, list) and head(c) == name]


def child(node, name):
    found = children(node, name)
    return found[0] if found else None


def floats(node, start=1):
    return [float(v) for v in node[start:] if not isinstance(v, list)]
