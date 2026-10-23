"""Footprints and pads from a .kicad_pcb file, placed with KiCad's own transform.

The transform is written out here instead of asking pcbnew, so the checker
does not trust the tool that made the board. It was checked against pcbnew
on the V2 HE board (every pad of every footprint, to 1 um):

    a footprint on B.Cu stores its pads with Y already mirrored, so
    board = position + R(angle) * stored, with R the KiCad rotation
    (Y down, positive angle counter-clockwise on screen):
        x' =  x cos a + y sin a
        y' = -x sin a + y cos a
"""

import math
from dataclasses import dataclass, field

from . import sexpr as S


def rotate(x, y, deg):
    a = math.radians(deg)
    return x * math.cos(a) + y * math.sin(a), -x * math.sin(a) + y * math.cos(a)


@dataclass
class Pad:
    number: str
    local: tuple          # as written for a top-side footprint (library frame, Y down)
    board: tuple          # board coordinates (KiCad, mm, Y down)
    size: tuple
    kind: str             # smd, thru_hole, np_thru_hole, connect
    layers: list
    drill: float = 0.0
    net: str = ""


@dataclass
class Footprint:
    ref: str
    lib: str
    pos: tuple
    angle: float
    bottom: bool
    pads: list = field(default_factory=list)
    board_only: bool = False
    dnp: bool = False
    exclude_pos: bool = False

    def pad(self, number):
        return [p for p in self.pads if p.number == number]


def load(path):
    tree = S.parse(open(path, encoding="utf-8").read())
    aux = S.child(S.child(tree, "setup") or [], "aux_axis_origin")
    origin = tuple(S.floats(aux)) if aux else (0.0, 0.0)
    fps = []
    for node in S.children(tree, "footprint"):
        at = S.floats(S.child(node, "at"))
        x, y, angle = at[0], at[1], (at[2] if len(at) > 2 else 0.0)
        layer = S.child(node, "layer")[1]
        bottom = layer == "B.Cu"
        ref = ""
        for prop in S.children(node, "property"):
            if prop[1] == "Reference":
                ref = str(prop[2])
        if not ref:
            ref = next((str(t[2]) for t in S.children(node, "fp_text") if t[1] == "reference"), "")
        attr = S.child(node, "attr") or []
        fp = Footprint(ref, str(node[1]), (x, y), angle, bottom,
                       board_only="board_only" in attr,
                       dnp="dnp" in attr or bool(S.child(node, "dnp")) and S.child(node, "dnp")[1:] != ["no"],
                       exclude_pos="exclude_from_pos_files" in attr)
        for p in S.children(node, "pad"):
            pat = S.floats(S.child(p, "at"))
            sx, sy = pat[0], pat[1]
            bx, by = rotate(sx, sy, angle)
            size = tuple(S.floats(S.child(p, "size"))) if S.child(p, "size") else (0, 0)
            layers = [str(v) for v in (S.child(p, "layers") or [None])[1:]]
            drill_node = S.child(p, "drill")
            drill = 0.0
            if drill_node:
                nums = [v for v in drill_node[1:] if not isinstance(v, list) and v != "oval"]
                drill = float(nums[0]) if nums else 0.0
            net_node = S.child(p, "net")
            net = str(net_node[-1]) if net_node else ""
            local = (sx, -sy) if bottom else (sx, sy)
            fp.pads.append(Pad(str(p[1]), local, (x + bx, y + by), size, str(p[2]), layers, drill, net))
        fps.append(fp)
    return origin, fps
