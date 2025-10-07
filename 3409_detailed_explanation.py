"""
Detailed Explanation of 0x3409 Compression Format with Real Data Examples

This file provides a comprehensive explanation of how the 0x3409 compression format
works for Vector3 data in Smash Ultimate animation files, using real data from
your terminal output.
"""

import struct

# Raw data from your terminal selection (0x3409 format)
raw_data = [9, 52, 0, 0, 40, 0, 0, 0, 0, 0, 128, 63, 91, 254, 230, 65, 2, 0, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 185, 194, 75, 63, 0, 0, 0, 0, 0, 0, 0, 0, 45, 99, 143, 66, 0, 0, 0, 0, 0, 0, 0, 0, 127, 89, 181, 66, 1, 0, 1, 0, 1, 0, 8, 18, 1, 0, 1, 0, 1, 0, 8, 18, 255, 255, 227, 11, 88, 113, 0, 18, 1, 128, 41, 53, 167, 15, 154, 14, 4, 96, 253, 127, 122, 49, 3, 67, 166, 27, 228, 43, 182, 14, 1, 32, 54, 24, 139, 68, 106, 15, 73, 52, 139, 8, 203, 42, 94, 2, 67, 36, 114, 254, 54,
30, 80, 252, 45, 25, 15, 250, 1, 22, 75, 247, 166, 19, 245, 17, 244, 14, 1, 0, 1, 0, 1, 0, 2, 17, 1, 0, 1, 0, 1, 0, 2, 17, 181, 4, 172, 20, 255, 17, 0, 17, 6, 128, 151, 246, 193, 47, 111, 11, 199, 127, 24, 190]

def explain_3409_format():
    """Explain the 0x3409 format structure and decompression process"""

    print("=" * 80)
    print("DETAILED EXPLANATION: 0x3409 Vector3 Compression Format")
    print("=" * 80)
    print()

    # Step 1: Parse the header
    print("STEP 1: Header Parsing")
    print("-" * 40)

    header_bytes = raw_data[0:4]
    header = struct.unpack('<I', bytes(header_bytes))[0]
    print(f"Raw header bytes: {header_bytes} → 0x{header:08X} (0x3409)")

    frame_count_bytes = raw_data[4:8]
    frame_count = struct.unpack('<I', bytes(frame_count_bytes))[0]
    print(f"Frame count bytes: {frame_count_bytes} → {frame_count} frames")

    unk1_bytes = raw_data[8:12]
    unk1 = struct.unpack('<f', bytes(unk1_bytes))[0]
    print(f"Unknown value 1 bytes: {unk1_bytes} → {unk1} (typically 1.0)")

    unk2_bytes = raw_data[12:16]
    unk2 = struct.unpack('<f', bytes(unk2_bytes))[0]
    print(f"Unknown value 2 bytes: {unk2_bytes} → {unk2:.6f}")

    flags_bytes = raw_data[16:18]
    flags = struct.unpack('<H', bytes(flags_bytes))[0]
    print(f"Flags bytes: {flags_bytes} → {flags} (compression flags)")

    bits_per_entry_bytes = raw_data[18:20]
    bits_per_entry = struct.unpack('<H', bytes(bits_per_entry_bytes))[0]
    print(f"Bits per entry bytes: {bits_per_entry_bytes} → {bits_per_entry} bits")
    print()

    # Step 2: Parse the keyframes
    print("STEP 2: Keyframe Vector3 Values")
    print("-" * 40)

    keyframes = []
    pos = 20  # Start after header (20 bytes)

    for i, name in enumerate(["First", "Middle", "Last"]):
        x_bytes = raw_data[pos:pos+4]
        y_bytes = raw_data[pos+4:pos+8]
        z_bytes = raw_data[pos+8:pos+12]

        x = struct.unpack('<f', bytes(x_bytes))[0]
        y = struct.unpack('<f', bytes(y_bytes))[0]
        z = struct.unpack('<f', bytes(z_bytes))[0]

        print(f"{name} keyframe:")
        print(f"  X: {x_bytes} → {x:.6f}")
        print(f"  Y: {y_bytes} → {y:.6f}")
        print(f"  Z: {z_bytes} → {z:.6f}")
        print(f"  Vector3: ({x:.6f}, {y:.6f}, {z:.6f})")
        print()

        keyframes.append((x, y, z))
        pos += 12

    first, middle, last = keyframes

    # Step 3: Compressed data
    print("STEP 3: Compressed Data")
    print("-" * 40)

    compressed_data = raw_data[pos:]
    print(f"Compressed data starts at byte position: {pos}")
    print(f"Compressed data length: {len(compressed_data)} bytes")
    print(f"First 32 bytes of compressed data: {compressed_data[:32]}")
    print()

    # Step 4: Interpolation logic explanation
    print("STEP 4: Interpolation Logic")
    print("-" * 40)

    print("The 0x3409 format uses SEGMENTED LINEAR INTERPOLATION with 3 keyframes:")
    print("- First keyframe: Start of animation sequence")
    print("- Middle keyframe: Middle of animation sequence")
    print("- Last keyframe: End of animation sequence")
    print()

    print("Interpolation is divided into TWO SEGMENTS:")
    print("1. t ∈ [0.0, 0.5]: Interpolate between 'first' and 'middle' keyframes")
    print("2. t ∈ [0.5, 1.0]: Interpolate between 'middle' and 'last' keyframes")
    print()

    def interpolate_component(start, middle, end, t):
        """Segmented interpolation function"""
        if t <= 0.5:
            # First segment: first to middle
            local_t = t * 2.0  # Map [0,0.5] to [0,1]
            return start + (middle - start) * local_t
        else:
            # Second segment: middle to last
            local_t = (t - 0.5) * 2.0  # Map [0.5,1.0] to [0,1]
            return middle + (end - middle) * local_t

    # Step 5: Real data interpolation examples
    print("STEP 5: Real Data Interpolation Examples")
    print("-" * 40)

    print("Using Z-component from your data (most interesting values):")
    z_first, z_middle, z_last = first[2], middle[2], last[2]
    print(f"Z values: first={z_first:.6f}, middle={z_middle:.6f}, last={z_last:.6f}")
    print()

    print("Interpolation table for Z-component:")
    print("t-value    Segment          Interpolated Value")
    print("--------   --------------   ------------------")

    test_t_values = [0.0, 0.1, 0.25, 0.4, 0.5, 0.6, 0.75, 0.9, 1.0]
    for t in test_t_values:
        interpolated = interpolate_component(z_first, z_middle, z_last, t)
        segment = "first→middle" if t <= 0.5 else "middle→last"
        print("4.2f")

    print()

    # Step 6: Index conversion
    print("STEP 6: Index to Interpolation Parameter Conversion")
    print("-" * 40)

    print("Each component (X, Y, Z) is stored as an INDEX value in the compressed data.")
    print("This index is converted to interpolation parameter t as follows:")
    print("t = index / (2^bits_per_component - 1)")
    print()

    # For 0x3409, bits_per_entry is 21, but each component typically uses 8 bits
    bits_per_component = 8  # Common for 0x3409
    max_index = (1 << bits_per_component) - 1

    print(f"Assuming {bits_per_component} bits per component (common for 0x3409):")
    print(f"Max index value: {max_index}")
    print()

    print("Example index conversions:")
    print("Index    t-value    Z-interpolated")
    print("-----    -------    --------------")

    example_indices = [0, 32, 64, 96, 128, 160, 192, 224, 255]
    for index in example_indices:
        t = index / max_index
        z_interp = interpolate_component(z_first, z_middle, z_last, t)
        print("5d")

    print()

    # Step 7: Complete decompression process
    print("STEP 7: Complete Decompression Process")
    print("-" * 40)

    print("To decompress one frame of Vector3 data:")
    print("1. Read 3 indices from compressed bit stream (one for X, Y, Z each)")
    print("2. Convert each index to t-value: t = index / (2^bits_per_component - 1)")
    print("3. For each component, interpolate using the segmented approach:")
    print("   - if t <= 0.5: result = first + (middle - first) * (t * 2)")
    print("   - if t > 0.5:  result = middle + (last - middle) * ((t - 0.5) * 2)")
    print("4. Combine X, Y, Z components into final Vector3")
    print("5. Repeat for each frame")
    print()

    print("This process reconstructs smooth animation curves while using minimal storage space!")

if __name__ == "__main__":
    explain_3409_format()
