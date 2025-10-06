import struct

# Read the binary file
with open('4300.bin', 'rb') as f:
    data = f.read()

print('Total data length:', len(data))
print('Hex dump:', data.hex())
print()

# Parse according to 0x4300 format - let's analyze
offset = 0

# Magic number
magic = struct.unpack('<I', data[offset:offset+4])[0]
print(f'Magic: 0x{magic:04X}')
offset += 4

# compressed_frame_count: u32
compressed_frame_count = struct.unpack('<I', data[offset:offset+4])[0]
print(f'compressed_frame_count: {compressed_frame_count}')
offset += 4

# unk1: f32
unk1 = struct.unpack('<f', data[offset:offset+4])[0]
print(f'unk1: {unk1}')
offset += 4

print(f'After unk1, offset: 0x{offset:02X}, remaining: {len(data) - offset}')

# Let's examine the remaining data
remaining = data[offset:]
print('Remaining data (first 50 bytes):', remaining[:50].hex())

# Try to understand the pattern
print('Remaining data as bytes:', list(remaining[:20]))

# Check if there are any recognizable patterns
print('Data as 32-bit values:')
for i in range(0, min(len(remaining), 40), 4):
    if i + 4 <= len(remaining):
        val = struct.unpack('<I', remaining[i:i+4])[0]
        print(f'  0x{i:02X}: 0x{val:08X}')

print('Data as 16-bit values:')
for i in range(0, min(len(remaining), 40), 2):
    if i + 2 <= len(remaining):
        val = struct.unpack('<H', remaining[i:i+2])[0]
        print(f'  0x{i:02X}: 0x{val:04X}')

print('Data as floats:')
for i in range(0, min(len(remaining), 40), 4):
    if i + 4 <= len(remaining):
        val = struct.unpack('<f', remaining[i:i+4])[0]
        print(f'  0x{i:02X}: {val}')

print()
print('Possible interpretations:')

# Option 1: Single quaternion per frame (4 floats = 16 bytes per frame)
print('Option 1: Quaternions (4 floats per frame)')
frames = compressed_frame_count
bytes_per_frame = 16
if len(remaining) >= frames * bytes_per_frame:
    for frame in range(frames):
        start = frame * bytes_per_frame
        if start + 16 <= len(remaining):
            q = struct.unpack('<4f', remaining[start:start+16])
            print(f'  Frame {frame}: {q}')
else:
    print('  Not enough data for quaternions')

# Option 2: Euler angles (3 floats = 12 bytes per frame)
print('Option 2: Euler angles (3 floats per frame)')
frames = compressed_frame_count
bytes_per_frame = 12
if len(remaining) >= frames * bytes_per_frame:
    for frame in range(frames):
        start = frame * bytes_per_frame
        if start + 12 <= len(remaining):
            euler = struct.unpack('<3f', remaining[start:start+12])
            print(f'  Frame {frame}: {euler}')
else:
    print('  Not enough data for euler angles')

# Option 3: Check for pattern like other compressed formats
print('Option 3: Check for compression headers')
if len(remaining) >= 4:
    possible_flags = struct.unpack('<H', remaining[0:2])[0]
    possible_bits = struct.unpack('<H', remaining[2:4])[0]
    print(f'  Possible flags: 0x{possible_flags:04X}')
    print(f'  Possible bits_per_entry: {possible_bits}')
    print(f'  Remaining data after header: {len(remaining) - 4} bytes')

# Option 4: Raw data without compression
print('Option 4: Raw data interpretation')
print('  First few floats:', [struct.unpack('<f', remaining[i:i+4])[0] for i in range(0, min(len(remaining), 32), 4) if i+4 <= len(remaining)])
