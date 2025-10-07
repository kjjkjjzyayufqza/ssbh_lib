"""
Simple Example: 0x3409 Vector3 Compression Logic

Using your real data to show exactly how 0x3409 compression works.
"""

import struct

# Your raw data from terminal
raw_data = [9, 52, 0, 0, 40, 0, 0, 0, 0, 0, 128, 63, 91, 254, 230, 65, 2, 0, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 185, 194, 75, 63, 0, 0, 0, 0, 0, 0, 0, 0, 45, 99, 143, 66, 0, 0, 0, 0, 0, 0, 0, 0, 127, 89, 181, 66, 1, 0, 1, 0, 1, 0, 8, 18, 1, 0, 1, 0, 1, 0, 8, 18, 255, 255, 227, 11, 88, 113, 0, 18, 1, 128, 41, 53, 167, 15, 154, 14, 4, 96, 253, 127, 122, 49, 3, 67, 166, 27, 228, 43, 182, 14, 1, 32, 54, 24, 139, 68, 106, 15, 73, 52, 139, 8, 203, 42, 94, 2, 67, 36, 114, 254, 54,
30, 80, 252, 45, 25, 15, 250, 1, 22, 75, 247, 166, 19, 245, 17, 244, 14, 1, 0, 1, 0, 1, 0, 2, 17, 1, 0, 1, 0, 1, 0, 2, 17, 181, 4, 172, 20, 255, 17, 0, 17, 6, 128, 151, 246, 193, 47, 111, 11, 199, 127, 24, 190]

def main():
    print("=== 0x3409 COMPRESSION: STEP-BY-STEP EXAMPLE ===")
    print()

    # 1. Parse header
    header, frame_count, unk1, unk2, flags, bits_per_entry = parse_header(raw_data)
    print(f"Header: 0x{header:08X}, Frames: {frame_count}, Bits per entry: {bits_per_entry}")

    # 2. Parse keyframes
    first, middle, last = parse_keyframes(raw_data)
    print(f"First keyframe:  ({first[0]:.6f}, {first[1]:.6f}, {first[2]:.6f})")
    print(f"Middle keyframe: ({middle[0]:.6f}, {middle[1]:.6f}, {middle[2]:.6f})")
    print(f"Last keyframe:   ({last[0]:.6f}, {last[1]:.6f}, {last[2]:.6f})")
    print()

    # 3. Show interpolation logic with REAL DATA
    print("=== KEY INSIGHT: Segmented Interpolation ===")
    print("0x3409 uses 3 keyframes and splits interpolation into 2 segments:")
    print("- t=0.0 to 0.5: interpolate between FIRST and MIDDLE keyframes")
    print("- t=0.5 to 1.0: interpolate between MIDDLE and LAST keyframes")
    print()

    # Use Z component (has interesting values)
    z_first, z_middle, z_last = first[2], middle[2], last[2]
    print(f"Using Z-component values: {z_first:.6f} → {z_middle:.6f} → {z_last:.6f}")
    print()

    print("INTERPOLATION EXAMPLES:")
    print("t     Segment         Calculation                     Result")
    print("----  --------------  -----------------------------  --------")

    test_values = [0.0, 0.25, 0.5, 0.75, 1.0]
    for t in test_values:
        if t <= 0.5:
            # First segment
            local_t = t * 2.0
            result = z_first + (z_middle - z_first) * local_t
            segment = "First→Middle"
            calc = ".6f"
        else:
            # Second segment
            local_t = (t - 0.5) * 2.0
            result = z_middle + (z_last - z_middle) * local_t
            segment = "Middle→Last"
            calc = ".6f"

        print("4.2f")

    print()
    print("=== HOW INDICES WORK ===")
    print("In compressed data, each component is stored as an INDEX (0-255)")
    print("Index is converted to interpolation parameter t:")
    print("t = index / 255  (for 8-bit indices)")
    print()

    print("INDEX → t → INTERPOLATED VALUE examples:")
    print("Index    t       Z-Value")
    print("-----  ------  ----------")

    # Show how indices map to actual interpolated values
    example_indices = [0, 64, 128, 192, 255]
    for index in example_indices:
        t = index / 255.0
        z_value = interpolate_segmented(z_first, z_middle, z_last, t)
        print("5d")

    print()
    print("=== COMPLETE DECOMPRESSION PROCESS ===")
    print("1. Read 3 indices from compressed bit stream (X, Y, Z)")
    print("2. Convert each index to t: t = index / 255")
    print("3. For each component, use segmented interpolation:")
    print("   if t <= 0.5: value = first + (middle - first) * (t * 2)")
    print("   if t > 0.5:  value = middle + (last - middle) * ((t - 0.5) * 2)")
    print("4. Combine into Vector3(x, y, z)")
    print()
    print("This is exactly how your 40-frame animation is compressed!")

def parse_header(data):
    """Parse 0x3409 header"""
    header = struct.unpack('<I', bytes(data[0:4]))[0]
    frame_count = struct.unpack('<I', bytes(data[4:8]))[0]
    unk1 = struct.unpack('<f', bytes(data[8:12]))[0]
    unk2 = struct.unpack('<f', bytes(data[12:16]))[0]
    flags = struct.unpack('<H', bytes(data[16:18]))[0]
    bits_per_entry = struct.unpack('<H', bytes(data[18:20]))[0]
    return header, frame_count, unk1, unk2, flags, bits_per_entry

def parse_keyframes(data):
    """Parse the 3 keyframes"""
    keyframes = []
    pos = 20
    for _ in range(3):
        x = struct.unpack('<f', bytes(data[pos:pos+4]))[0]
        y = struct.unpack('<f', bytes(data[pos+4:pos+8]))[0]
        z = struct.unpack('<f', bytes(data[pos+8:pos+12]))[0]
        keyframes.append((x, y, z))
        pos += 12
    return keyframes

def interpolate_segmented(first, middle, last, t):
    """Segmented interpolation used by 0x3409"""
    if t <= 0.5:
        local_t = t * 2.0
        return first + (middle - first) * local_t
    else:
        local_t = (t - 0.5) * 2.0
        return middle + (last - middle) * local_t

if __name__ == "__main__":
    main()
