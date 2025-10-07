import struct

# Raw data from terminal selection
raw_data = [9, 52, 0, 0, 40, 0, 0, 0, 0, 0, 128, 63, 91, 254, 230, 65, 2, 0, 21, 0, 0, 0, 0, 0, 0, 0, 0, 0, 185, 194, 75, 63, 0, 0, 0, 0, 0, 0, 0, 0, 45, 99, 143, 66, 0, 0, 0, 0, 0, 0, 0, 0, 127, 89, 181, 66, 1, 0, 1, 0, 1, 0, 8, 18, 1, 0, 1, 0, 1, 0, 8, 18, 255, 255, 227, 11, 88, 113, 0, 18, 1, 128, 41, 53, 167, 15, 154, 14, 4, 96, 253, 127, 122, 49, 3, 67, 166, 27, 228, 43, 182, 14, 1, 32, 54, 24, 139, 68, 106, 15, 73, 52, 139, 8, 203, 42, 94, 2, 67, 36, 114, 254, 54,
30, 80, 252, 45, 25, 15, 250, 1, 22, 75, 247, 166, 19, 245, 17, 244, 14, 1, 0, 1, 0, 1, 0, 2, 17, 1, 0, 1, 0, 1, 0, 2, 17, 181, 4, 172, 20, 255, 17, 0, 17, 6, 128, 151, 246, 193, 47, 111, 11, 199, 127, 24, 190]

def parse_3409_header(data):
    """Parse the 0x3409 format header"""
    header = struct.unpack('<I', bytes(data[0:4]))[0]
    frame_count = struct.unpack('<I', bytes(data[4:8]))[0]
    unk1 = struct.unpack('<f', bytes(data[8:12]))[0]
    unk2 = struct.unpack('<f', bytes(data[12:16]))[0]
    flags = struct.unpack('<H', bytes(data[16:18]))[0]
    bits_per_entry = struct.unpack('<H', bytes(data[18:20]))[0]

    return header, frame_count, unk1, unk2, flags, bits_per_entry

def parse_default_vectors(data, count=3):
    """Parse the default Vector3 values (keyframes)"""
    vectors = []
    pos = 20  # Start after header
    for i in range(count):
        x = struct.unpack('<f', bytes(data[pos:pos+4]))[0]
        y = struct.unpack('<f', bytes(data[pos+4:pos+8]))[0]
        z = struct.unpack('<f', bytes(data[pos+8:pos+12]))[0]
        vectors.append((x, y, z))
        pos += 12
    return vectors, pos

def interpolate_component(start, middle, end, t):
    """
    Interpolate between three keyframes using segmented approach.
    First half (t=0.0-0.5): interpolate between start and middle
    Second half (t=0.5-1.0): interpolate between middle and end
    """
    if t <= 0.5:
        # First segment: start to middle
        local_t = t * 2.0
        return start + (middle - start) * local_t
    else:
        # Second segment: middle to end
        local_t = (t - 0.5) * 2.0
        return middle + (end - middle) * local_t

def demonstrate_3409_compression_logic():
    """Demonstrate 0x3409 compression logic with actual data"""

    print("=== 0x3409 Format Analysis with Real Data ===")
    print()

    # Parse header
    header, frame_count, unk1, unk2, flags, bits_per_entry = parse_3409_header(raw_data)
    print(f"Header: 0x{header:08X}")
    print(f"Frame Count: {frame_count}")
    print(f"Unknown Value 1: {unk1}")
    print(f"Unknown Value 2: {unk2}")
    print(f"Flags: {flags}")
    print(f"Bits Per Entry: {bits_per_entry}")
    print()

    # Parse default vectors (keyframes)
    default_vectors, compressed_start = parse_default_vectors(raw_data)
    first, middle, last = default_vectors

    print("Key Frame Vectors:")
    print(f"First:  ({first[0]:.6f}, {first[1]:.6f}, {first[2]:.6f})")
    print(f"Middle: ({middle[0]:.6f}, {middle[1]:.6f}, {middle[2]:.6f})")
    print(f"Last:   ({last[0]:.6f}, {last[1]:.6f}, {last[2]:.6f})")
    print()

    # Show compressed data info
    compressed_data = raw_data[compressed_start:]
    print(f"Compressed data starts at position: {compressed_start}")
    print(f"Compressed data length: {len(compressed_data)} bytes")
    print(f"Compressed data (first 20 bytes): {compressed_data[:20]}")
    print()

    print("=== Interpolation Logic Explanation ===")
    print("0x3409 uses segmented interpolation with 3 keyframes:")
    print("- Range 0.0-0.5: Linear interpolation between 'first' and 'middle'")
    print("- Range 0.5-1.0: Linear interpolation between 'middle' and 'last'")
    print()

    # Demonstrate interpolation with actual keyframe data
    print("X-component interpolation examples using actual keyframes:")
    x_first, x_middle, x_last = first[0], middle[0], last[0]
    print(f"X values: first={x_first:.6f}, middle={x_middle:.6f}, last={x_last:.6f}")

    for t in [0.0, 0.25, 0.5, 0.75, 1.0]:
        interpolated = interpolate_component(x_first, x_middle, x_last, t)
        segment = "first→middle" if t <= 0.5 else "middle→last"
        print(f"  t={t:4.2f} ({segment:12s}): {interpolated:.6f}")

    print()
    print("Y-component interpolation examples:")
    y_first, y_middle, y_last = first[1], middle[1], last[1]
    print(f"Y values: first={y_first:.6f}, middle={y_middle:.6f}, last={y_last:.6f}")

    for t in [0.0, 0.25, 0.5, 0.75, 1.0]:
        interpolated = interpolate_component(y_first, y_middle, y_last, t)
        segment = "first→middle" if t <= 0.5 else "middle→last"
        print(f"  t={t:4.2f} ({segment:12s}): {interpolated:.6f}")

    print()
    print("=== Decompression Process ===")
    print("1. Read compressed bit stream from compressed data")
    print("2. Extract indices for each component (X, Y, Z) using bits_per_entry")
    print("3. Convert indices to interpolation parameter t:")
    print("   t = index / (2^bits_per_component - 1)")
    print("4. Use t to interpolate between the 3 keyframes")
    print("5. Each frame requires 3 indices (one per component)")
    print()

    # Calculate bits per component (assuming 8 bits as used in the code)
    bits_per_component = 8  # Common value for 0x3409
    max_index = (1 << bits_per_component) - 1

    print(f"=== Index to Interpolation Parameter Conversion ===")
    print(f"Bits per component: {bits_per_component}")
    print(f"Max index value: {max_index}")
    print("Example conversions:")

    for index in [0, 64, 128, 192, 255]:
        t = index / max_index
        interpolated_x = interpolate_component(x_first, x_middle, x_last, t)
        print(f"  Index {index:3d} → t={t:.3f} → X={interpolated_x:.6f}")

if __name__ == "__main__":
    demonstrate_3409_compression_logic()
