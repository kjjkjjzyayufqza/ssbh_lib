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

## Key Insights

- **Efficiency**: Stores only 3 keyframes instead of 40 individual frames
- **Quality**: Segmented interpolation provides better precision than simple linear interpolation
- **Flexibility**: Can represent complex curves with minimal data
- **Application**: Perfect for smooth animation curves in game assets

## Compression Benefits

For the analyzed 40-frame animation:
- **Uncompressed**: Would require 40 × 3 × 4 = 480 bytes
- **Compressed**: Uses ~56 bytes header + 3 keyframes (36 bytes) + compressed indices (~120 bytes) = ~212 bytes
- **Space savings**: ~56% reduction while maintaining visual quality

This compression format is specifically designed for the variable-rate nature of animation data, where keyframes are more important than intermediate values.
