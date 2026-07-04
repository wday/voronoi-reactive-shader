"""Parse Resolume .avc (XML) files into Block, Source, and Param records."""

from __future__ import annotations

import xml.etree.ElementTree as ET
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class Block:
    type: str
    name: str
    blend: str | None = None
    opacity: float | None = None
    params: dict = field(default_factory=dict)
    scope: str = "comp"
    layer: int | None = None
    clip: str | None = None
    col: int | None = None
    effect_index: int | None = None  # 1-indexed position in chain (OSC-style)
    osc: str = ""  # base OSC path for this effect
    file: str = ""


@dataclass
class Param:
    """A single controllable parameter with its full OSC address."""
    name: str
    value: float | int | str | None = None
    osc: str = ""  # full OSC path: .../effects/{e}/param{p} or .../effects/{e}/opacity
    effect_type: str = ""
    effect_name: str = ""
    scope: str = "comp"
    layer: int | None = None
    clip: str | None = None
    col: int | None = None
    file: str = ""


@dataclass
class Source:
    source_type: str  # feedback, generator, capture, file
    source_name: str
    scope: str = "clip"
    layer: int | None = None
    clip: str | None = None
    col: int | None = None
    file: str = ""


def parse_avc(path: str | Path) -> tuple[list[Block], list[Source]]:
    tree = ET.parse(path)
    root = tree.getroot()

    blocks: list[Block] = []
    sources: list[Source] = []
    ctx = {"scope": "comp", "layer": None, "clip": None, "col": None}

    _walk(root, ctx, blocks, sources)

    filename = Path(path).stem
    for b in blocks:
        b.file = filename
    for s in sources:
        s.file = filename

    return blocks, sources


def expand_params(blocks: list[Block]) -> list[Param]:
    """Expand blocks into individual Param records with OSC addresses."""
    params: list[Param] = []
    for b in blocks:
        if not b.osc:
            continue

        # Effect opacity/mix
        if b.opacity is not None:
            params.append(Param(
                name="opacity",
                value=b.opacity,
                osc=f"{b.osc}/opacity",
                effect_type=b.type,
                effect_name=b.name,
                scope=b.scope,
                layer=b.layer,
                clip=b.clip,
                col=b.col,
                file=b.file,
            ))

        # Numbered params — Resolume exposes these as param1, param2, ...
        param_idx = 1
        for pname, pval in b.params.items():
            if pname in ("Name",):
                continue
            params.append(Param(
                name=pname,
                value=pval,
                osc=f"{b.osc}/param{param_idx}",
                effect_type=b.type,
                effect_name=b.name,
                scope=b.scope,
                layer=b.layer,
                clip=b.clip,
                col=b.col,
                file=b.file,
            ))
            param_idx += 1

    return params


# -- tree walk ----------------------------------------------------------------


def _walk(el, ctx: dict, blocks: list[Block], sources: list[Source]):
    tag = el.tag

    if tag == "Layer":
        ctx = {**ctx, "scope": "layer", "layer": _int(el.get("layerIndex"))}
    elif tag == "Clip":
        name_p = el.find('./Params/Param[@name="Name"]')
        clip_name = name_p.get("value", "") if name_p is not None else ""
        ctx = {
            **ctx,
            "scope": "clip",
            "clip": clip_name,
            "col": _int(el.get("columnIndex")),
            "layer": _int(el.get("layerIndex", str(ctx.get("layer", -1)))),
        }

    # Extract blocks and sources from VideoTrack
    if tag == "VideoTrack":
        chain = el.find("RenderPass")  # RenderPassChain container
        if chain is not None:
            _extract_chain(chain, ctx, blocks)
        # VideoSource lives under VideoTrack → PrimarySource
        for vs in el.iter("VideoSource"):
            _extract_source(vs, ctx, sources)
        return  # don't recurse further

    for child in el:
        _walk(child, ctx, blocks, sources)


# -- OSC path construction ----------------------------------------------------


def _osc_base(ctx: dict) -> str:
    """Build the OSC path prefix from context (scope + indices)."""
    # Convert 0-indexed XML to 1-indexed OSC
    if ctx["scope"] == "comp":
        return "/composition/video"
    elif ctx["scope"] == "layer" and ctx["layer"] is not None:
        return f"/composition/layers/{ctx['layer'] + 1}/video"
    elif ctx["scope"] == "clip" and ctx["layer"] is not None and ctx["col"] is not None:
        return f"/composition/layers/{ctx['layer'] + 1}/clips/{ctx['col'] + 1}/video"
    return ""


# -- block extraction ---------------------------------------------------------


def _extract_chain(chain, ctx: dict, blocks: list[Block]):
    osc_prefix = _osc_base(ctx)
    effect_idx = 0  # 0-based counter, skipping transforms

    for rp in chain:
        if rp.tag != "RenderPass":
            continue

        base = rp.get("baseType", "")
        rp_type = rp.get("type", "")

        if rp_type == "TransformEffect":
            continue

        effect_idx += 1  # 1-indexed for OSC
        osc_path = f"{osc_prefix}/effects/{effect_idx}" if osc_prefix else ""

        if base == "DryWetEffect":
            _extract_drywet(rp, ctx, blocks, effect_idx, osc_path)
        elif base == "Effect":
            params = _read_params(rp)
            opacity = params.pop("Opacity", None)
            blocks.append(Block(
                type=rp_type,
                name=rp.get("name", ""),
                opacity=opacity,
                params=params,
                effect_index=effect_idx,
                osc=osc_path,
                **ctx,
            ))


def _extract_drywet(rp, ctx: dict, blocks: list[Block],
                    effect_idx: int, osc_path: str):
    inner_type = ""
    inner_name = rp.get("name", "")
    blend = None
    opacity = None
    params: dict = {}

    for child in rp:
        if child.tag == "RenderPass" and child.get("dwType") == "Effect":
            inner_type = child.get("type", "")
            params = _read_params(child)
        elif child.tag == "ChoosableMixer":
            for mixer_rp in child.findall("RenderPass"):
                if mixer_rp.get("baseType") == "Mixer":
                    blend = mixer_rp.get("type")
                    mixer_params = _read_params(mixer_rp)
                    opacity = mixer_params.get("Opacity")

    # Skip DryWet-wrapped transforms (boilerplate)
    if inner_type == "TransformEffect":
        return

    # Fallback: wrapper-level opacity
    if opacity is None:
        wrapper_params = _read_params(rp)
        opacity = wrapper_params.get("Opacity")

    blocks.append(Block(
        type=inner_type or rp.get("type", ""),
        name=inner_name,
        blend=blend,
        opacity=opacity,
        params=params,
        effect_index=effect_idx,
        osc=osc_path,
        **ctx,
    ))


# -- source extraction --------------------------------------------------------


def _extract_source(vs, ctx: dict, sources: list[Source]):
    vtype = vs.get("type", "")

    if vtype == "VideoSourceFeedback":
        sources.append(Source(
            source_type="feedback", source_name="Feedback", **ctx,
        ))

    elif vtype == "GeneratorVideoSource":
        gen_name = ""
        for rp in vs.iter("RenderPass"):
            t = rp.get("type", "")
            if t and t != "TransformEffect":
                gen_name = t
                break
        sources.append(Source(
            source_type="generator", source_name=gen_name, **ctx,
        ))

    elif vtype == "CaptureDeviceVideoSource":
        device_id = ""
        for cs in vs.iter("CaptureSource"):
            device_id = cs.get("deviceId", "")
        # Extract friendly name from end of device ID
        friendly = device_id.rsplit("_", 1)[-1] if "_" in device_id else device_id
        sources.append(Source(
            source_type="capture", source_name=friendly, **ctx,
        ))

    elif vtype == "CompositionRouterVideoSource":
        input_ref = ""
        for params_el in vs.findall("Params"):
            for p in params_el:
                if p.get("name") == "Input":
                    input_ref = p.get("value", "")
        if input_ref:
            source_name = f"Route {input_ref}"
        else:
            source_name = "Layers Below"
        sources.append(Source(
            source_type="router", source_name=source_name, **ctx,
        ))

    elif vtype == "VideoFormatReaderSource":
        file_name = ""
        for vfr in vs.iter("VideoFormatReaderSource"):
            file_name = vfr.get("fileName", "")
        if file_name:
            file_name = file_name.replace("\\", "/").rsplit("/", 1)[-1]
        sources.append(Source(
            source_type="file", source_name=file_name, **ctx,
        ))


# -- helpers ------------------------------------------------------------------


def _read_params(el) -> dict:
    """Read ParamRange/ParamChoice/Param values from an element's Params child."""
    params: dict = {}
    for params_el in el.findall("Params"):
        for p in params_el:
            name = p.get("name", "")
            if not name:
                continue
            if p.tag == "ParamRange":
                try:
                    params[name] = float(p.get("value", "0"))
                except ValueError:
                    params[name] = p.get("value", "")
            elif p.tag == "ParamChoice":
                try:
                    params[name] = int(p.get("value", "0"))
                except ValueError:
                    params[name] = p.get("value", "")
            elif p.tag == "Param":
                params[name] = p.get("value", "")
    return params


def _int(val) -> int | None:
    if val is None:
        return None
    try:
        return int(val)
    except ValueError:
        return None
