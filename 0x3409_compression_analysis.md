# 0x3409 Vector3 Compression Format Analysis

## Overview

The 0x3409 format is a compression scheme used in Smash Ultimate animation files (.nuanmb) for compressing Vector3 data (typically translation/position values). It uses segmented linear interpolation with 3 keyframes to achieve efficient compression while maintaining smooth animation curves.

## Format Structure

### Header (20 bytes)
```
Offset  Size  Type  Description
0       4     u32   Magic: 0x00003409
4       4     u32   Frame count
8       4     f32   Unknown value 1 (typically 1.0)
12      4     f32   Unknown value 2 (varies)
16      2     u16   Flags (typically 2)
18      2     u16   Bits per entry
```

### Keyframes (36 bytes)
After the header, there are exactly 3 Vector3 keyframes:
- **First keyframe**: Start of animation sequence
- **Middle keyframe**: Middle of animation sequence
- **Last keyframe**: End of animation sequence

Each keyframe consists of 3 float32 values (X, Y, Z) for a total of 12 bytes per keyframe.

### Compressed Data
Following the keyframes is the compressed bit stream containing indices for each frame and component.

## Real Data Example

Using data from a GBL_RT translation track:

### Header Values
- Magic: `0x00003409`
- Frame count: `40` frames
- Unknown value 1: `1.0`
- Unknown value 2: `28.874197`
- Flags: `2`
- Bits per entry: `21`

### Keyframe Values
```
First:  (0.000000, 0.000000, 0.795940)
Middle: (0.000000, 0.000000, 71.693703)
Last:   (0.000000, 0.000000, 90.674797)
```

## Interpolation Algorithm

### Segmented Linear Interpolation

The 0x3409 format uses **segmented linear interpolation** with 3 keyframes, dividing the interpolation range into two segments:

#### Segment 1: t ∈ [0.0, 0.5]
Interpolate between **first** and **middle** keyframes:
```
local_t = t * 2.0  // Map [0, 0.5] to [0, 1]
result = first + (middle - first) * local_t
```

#### Segment 2: t ∈ [0.5, 1.0]
Interpolate between **middle** and **last** keyframes:
```
local_t = (t - 0.5) * 2.0  // Map [0.5, 1.0] to [0, 1]
result = middle + (last - middle) * local_t
```

### Z-Component Interpolation Examples

Using the real keyframe values (0.795940 → 71.693703 → 90.674797):

| t    | Segment       | Calculation                          | Result     |
|------|---------------|--------------------------------------|------------|
| 0.00 | First→Middle | 0.795940 + (71.693703 - 0.795940) * 0 | 0.795940  |
| 0.25 | First→Middle | 0.795940 + (71.693703 - 0.795940) * 0.5 | 36.244821 |
| 0.50 | First→Middle | 0.795940 + (71.693703 - 0.795940) * 1.0 | 71.693703 |
| 0.75 | Middle→Last  | 71.693703 + (90.674797 - 71.693703) * 0.5 | 81.184250 |
| 1.00 | Middle→Last  | 71.693703 + (90.674797 - 71.693703) * 1.0 | 90.674797 |

## Index to Parameter Conversion

Each component (X, Y, Z) is stored as an **index value** in the compressed data. For 8-bit indices (common for 0x3409):

```
t = index / 255  // Convert index to interpolation parameter
```

### Index Conversion Examples

| Index | t       | Z-Interpolated |
|-------|---------|----------------|
| 0     | 0.000   | 0.795940      |
| 64    | 0.251   | 18.571836     |
| 128   | 0.502   | 36.244821     |
| 192   | 0.753   | 81.184250     |
| 255   | 1.000   | 90.674797     |

## Decompression Process

To decompress one frame of Vector3 data:

1. **Read indices**: Extract 3 indices from compressed bit stream (one for X, Y, Z each)
2. **Convert to t**: `t = index / (2^bits_per_component - 1)`
3. **Interpolate each component** using segmented approach:
   - if t ≤ 0.5: `result = first + (middle - first) * (t * 2)`
   - if t > 0.5: `result = middle + (last - middle) * ((t - 0.5) * 2)`
4. **Combine**: Create final Vector3(x, y, z)
5. **Repeat** for each frame

## Key Insights (Updated with Function Analysis)

- **Efficiency**: Stores only 3 keyframes instead of 40 individual frames
- **Quality**: Segmented interpolation provides better precision than simple linear interpolation
- **Flexibility**: Can represent complex curves with minimal data
- **Application**: Perfect for smooth animation curves in game assets

---

## Function-Level Decompression Flow (IDA Analysis Result)

The high-level algorithm is executed through a nested object model:

1.  **Main Dispatcher (`sub_140244330`):** The VTable `operator()` entry point for the compressed track.
2.  **Interpolation Scheduler (`sub_140235330`):** Handles frame time decomposition, boundary checks (`<0.01f`, `>0.99f`), and dispatches to **Read Single Frame (RSF)** and **Interpolate Between Frames (IBF)** logic via function objects/structs stored in the Decompressor object (`a1+64` and `a1+136`).
3.  **Read Single Frame Core (`sub_14023FCF0`):** This function is the actual entry point for RSF logic. It acts as a **four-component dispatcher**, delegating X, Y, Z, and W processing to four distinct function pointers stored in its internal structure (`a1[4]`, `a1[15]`, `a1[26]`, `a1[37]`). These four functions contain the hard-coded BitReader (e.g., 21-bit index extraction) and the final inverse segmented interpolation computation.
4.  **Key Parameters:**
    *   **Normalization Factor:** The inverse of the total frame count is stored in the Decompressor structure at offset **+72**.
    *   **Keyframes:** 3 x Vector3 (K1, K2, K3) are passed as captured context to the Lambda object.

## Compression Benefits

For the analyzed 40-frame animation:
- **Uncompressed**: Would require 40 × 3 × 4 = 480 bytes
- **Compressed**: Uses ~56 bytes header + 3 keyframes (36 bytes) + compressed indices (~120 bytes) = ~212 bytes
- **Space savings**: ~56% reduction while maintaining visual quality

This compression format is specifically designed for the variable-rate nature of animation data, where keyframes are more important than intermediate values.


Translate track: GBL_RT
Translate header: 0x3409
Translate data: [9, 52, 0, 0, 40, 0, 0, 0, 0, 0, 128, 63, 91, 254, 230, 65, 2, 0, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 185, 194, 75, 63, 0, 0, 0, 0, 0, 0, 0, 0, 45, 99, 143, 66, 0, 0, 0, 0, 0, 0, 0, 0, 127, 89, 181, 66, 1, 0, 1, 0, 1, 0, 8, 18, 1, 0, 1, 0, 1, 0, 8, 18, 255, 255, 227, 11, 88, 113, 0, 18, 1, 128, 41, 53, 167, 15, 154, 14, 4, 96, 253, 127, 122, 49, 3, 67, 166, 27, 228, 43, 182, 14, 1, 32, 54, 24, 
139, 68, 106, 15, 73, 52, 139, 8, 203, 42, 94, 2, 67, 36, 114, 254, 54, 30, 80, 252, 45, 25, 15, 250, 1, 22, 75, 247, 166, 19, 245, 17, 
244, 14, 1, 0, 1, 0, 1, 0, 2, 17, 1, 0, 1, 0, 1, 0, 2, 17, 181, 4, 172, 20, 255, 17, 0, 17, 6, 128, 151, 246, 193, 47, 111, 11, 199, 127, 24, 190


compress data
1, 0, 1, 0, 1, 0, 8, 18, 1, 0, 1, 0, 1, 0, 8, 18, 255, 255, 227, 11, 88, 113, 0, 18, 1, 128, 41, 53, 167, 15, 154, 14, 4, 96, 253, 127, 122, 49, 3, 67, 166, 27, 228, 43, 182, 14, 1, 32, 54, 24, 
139, 68, 106, 15, 73, 52, 139, 8, 203, 42, 94, 2, 67, 36, 114, 254, 54, 30, 80, 252, 45, 25, 15, 250, 1, 22, 75, 247, 166, 19, 245, 17, 
244, 14, 1, 0, 1, 0, 1, 0, 2, 17, 1, 0, 1, 0, 1, 0, 2, 17, 181, 4, 172, 20, 255, 17, 0, 17, 6, 128, 151, 246, 193, 47, 111, 11, 199, 127, 24, 190

## Additional Example: 3409_3.bin (Translation on X Only)

Context: This file contains a 0x3409-compressed track where only X varies; Y and Z remain 0.

Header (parsed):
- Magic: 0x00003409
- Frame count: 45
- Unknown value 1: 1.0
- Unknown value 2: 6.900769
- Flags: 2
- Bits per entry: 15

Keyframes (Vector3):
- First:  (1.672210, 0.000000, 0.000000)
- Middle: (48.582199, 0.000000, 0.000000)
- Last:   (59.990200, 0.000000, 0.000000)

Compressed data overview:
- Total compressed bytes after keyframes: 108 bytes
- Detected segment header (duplicated u16 quartet): last u16 = 0x1208
  - Interpreting the last u16 as [high_byte | low_byte]: high_byte=0x12 (18), low_byte=0x08
  - Inferred per-segment bit width (for the active axis) = 18 bits
- Segment 0 payload: offset 60 (from start of file), length 48 bytes
  - Entries if pure single-axis stream: floor(48 bytes × 8 / 18 bits) = 21 entries

Notes and implications:
- Unlike the 40-frame Z-translation example, 3409_3.bin reports Bits per entry = 15 in the header, yet the segment-local header’s high byte (0x12) implies 18 bits for the active axis within this segment. This reinforces the model that the stream is segmented and each segment can specify its own packing parameters independent of the global header field.
- Only one duplicated header was detected within the compressed area, suggesting there may be a single segment (or the second is elsewhere/not present).
- Since only X varies, the inferred “active-axis bit width” should be applied to X when building per-frame indices. Y and Z likely compress to trivial/constant forms.
- To reconstruct, we follow the same segmented interpolation model using the three keyframes, but the time parameter t for each frame comes from decoding per-frame indices within this segment using the inferred bit width (18) and the segment’s ordering.

Open questions for this sample:
- Whether there are additional segment parameters (e.g., low byte 0x08) that influence intra-segment sampling (nonlinear spacing or LUT selection) remains to be validated by full-frame decoding and comparison to a ground-truth series.

## Additional Example: 3409_2.bin (X/Y/Z All Vary)

Header (parsed):
- Magic: 0x00003409
- Frame count: 67
- Unknown value 1: 1.0
- Unknown value 2: 17.935175
- Flags: 2
- Bits per entry: 46

Keyframes (Vector3):
- First:  (0.454533, -0.810477, 1.992870)
- Middle: (0.301214, -0.750987, 2.126730)
- Last:   (0.000000, 3.000000, 0.000000)

Compressed data:
- Total compressed bytes after keyframes: 320 bytes
- No duplicated per-segment small headers detected in this sample (unlike prior examples). This suggests either:
  - A single continuous segment without the duplicated header convention, or
  - A different packing flavor for multi-axis movement where control info is embedded differently.

Implications of bits_per_entry = 46:
- If evenly split per axis, that would be ~15.33 bits/axis, which is not an integer. This indicates the three components are not simply equal-width packed.
- Plausible layouts include asymmetric per-axis bit widths whose sum is 46 (e.g., 16/15/15 or 18/14/14, etc.), or interleaved variable-width codes.
- With no explicit duplicated segment header, per-axis widths may be defined implicitly by `bits_per_entry` and/or `flags`.

Next steps for this sample:
- Perform bit-level decoding trials to infer the exact (bx, by, bz) triplet summing to 46 that best reconstructs the ground truth curves (from the corresponding anim/JSON). This will help generalize the model beyond the segmented, single-axis-dominant case.
- Investigate whether the lack of duplicated headers correlates with "all axes active" tracks and if so, document the control field placement for per-entry packing.
