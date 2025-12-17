## Purpose

This document describes an AI-focused implementation plan to make JSON -> NUANMB (Anim v1.2) output load correctly in EXVS2, matching the game’s expectations for track/property semantics.

The goal is not a perfect, generic Anim v1.2 writer for all games. The goal is to produce files that EXVS2 loads and plays correctly, with minimal behavioral differences from known-good source assets.

## Observed Problem

When writing Anim v1.2 from JSON using a naive “always write Scale/Rotate/Translate for every bone” approach, the game loads the file but the character pose becomes incorrect (often collapsing toward the origin or behaving as if transforms are zeroed).

This is not a simple numeric mismatch. It is a semantic mismatch: the game interprets the authored properties differently than intended.

## Key Findings From a Known-Good Source File

Using a low-level dump of a known-good file:

- Many Transform tracks do not contain Translate or Scale properties at all.
- Some tracks contain only Rotate (plus optional CompensateScale and Visibility).
- Translate in the known-good file frequently uses compressed header 0x3409.
- Rotate can use 0x4409 for multi-frame data, and 0x4003 for single quaternion values.
- The Anim::V12 header fields (name, unk1, unk2, unk3) are not always zero in the source file.

Implication:

- The presence or absence of a property is meaningful.
- If a property is absent, the game likely falls back to skeleton rest pose defaults (or other internal defaults).
- Writing explicit Translate = (0,0,0) for bones that previously had no Translate can override the rest pose and break the animation.

## Design Goals

- Preserve property presence semantics: if a source track did not have Translate, the exported file must not introduce Translate.
- Preserve Anim::V12 header behavior: avoid hardcoding header fields to zero if the game relies on them.
- Produce buffers in formats the game actually supports.
- Provide deterministic, scriptable validation that compares outputs against known-good sources.

## Non-Goals

- Reconstructing every unknown field in the Anim v1.2 format for all games.
- Achieving binary-identical output.

## Implementation Plan (Phased)

### Phase 0: Build a Reliable Comparison Harness

Deliverables:

- A tool/script that reads two NUANMB files and reports:
  - Anim::V12 header fields and differences
  - Per-track property lists (names and buffer indices)
  - Per-buffer header (u32), size, and a short hex prefix
- A focused report for specific tracks (e.g., BASE, CENTER_RT, ASHI_L):
  - Which properties exist in source vs generated
  - What the property buffer headers are in source vs generated

Success criteria:

- The harness can explain any “game looks wrong” case by pointing to concrete semantic differences (property omission vs explicit override, header mismatch, unsupported buffer header).

### Phase 1: Preserve Property Presence Semantics End-to-End

Problem:

- The current high-level JSON representation always includes scale/rotation/translation per frame, but it may not encode whether the original file omitted a property.

Plan:

- Extend the high-level representation to encode whether Scale/Rotate/Translate are authored for a Transform track.
- Ensure the read path from a v1.2 NUANMB records property presence.
- Ensure the write path respects it: omit Scale/Translate/Rotate properties when they were not present.

Implementation detail:

- Use explicit booleans for authoring state, not inferred heuristics.
- The authoring state must roundtrip: source NUANMB -> JSON -> NUANMB preserves property lists.

Success criteria:

- The generated NUANMB has the same property list per track as the source file.
- Loading the generated file in the game no longer collapses bones due to accidental Translate/Scale overrides.

### Phase 2: Preserve Anim::V12 Header Fields That Affect Runtime

Problem:

- Source files may use non-zero values for Anim::V12 header fields.

Plan:

- Include Anim::V12 header fields in JSON for version 1.2.
- On write, reproduce these values rather than defaulting to zeros.

Success criteria:

- The generated NUANMB matches the source header fields, unless explicitly overridden.

### Phase 3: Translate Compatibility First (0x3409)

Goal:

- For tracks where Translate is present in the source, produce Translate buffers in a format the game supports.

Plan:

- Implement an encoder for 0x3409 Vector3 curves using the validated model:
  - 33-key blocks
  - endpoints + residual/kernel-based reconstruction
  - base_scale, flags, bits fields written to match the format expectations
- Validate encoder output by:
  - decoding the encoded buffer with the existing decoder and checking frame-wise error
  - comparing the property header and overall behavior against source in-game

Success criteria:

- Tracks with Translate present in the source play correctly in-game after JSON -> NUANMB.
- Frame-wise numeric error is within expected lossy compression tolerance.

### Phase 4: Scale Compatibility (Reuse Translate Encoder)

Goal:

- For tracks where Scale is present in the source, write Scale using the same vec3 encoding family used by the game.

Plan:

- Reuse the vec3 0x3409 encoder for Scale where appropriate.
- Preserve CompensateScale behavior and do not introduce Scale properties where absent.

Success criteria:

- Scale behavior matches the source in-game for bones with authored scale.

### Phase 5: Rotate Compatibility (0x4409)

Goal:

- For tracks where Rotate is multi-frame in the source, produce 0x4409 buffers.

Plan:

- Implement 0x4409 quaternion encoder (endpoints + residual + normalization) consistent with the validated decode model.
- Ensure quaternion continuity rules are handled:
  - normalization
  - sign consistency where required by the format or by game runtime

Success criteria:

- Multi-frame rotations match source behavior in-game.

## Validation Checklist

- Property presence parity: source vs generated per track.
- Header parity: Anim::V12 fields match expected values.
- Buffer header parity: Translate uses 0x3409 where source uses 0x3409; Rotate uses 0x4409 where source uses 0x4409.
- Runtime validation: the game loads the file and plays the animation without collapsing transforms.

## Current Status

- A decoder bug in 0x4300 offset handling was identified and fixed previously (raw quaternion stream header length variant).
- Further work should prioritize semantic parity (property omission and header fields) before format-level encoders.
