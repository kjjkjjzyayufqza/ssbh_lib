import struct

# Read the binary file
with open('3408.bin', 'rb') as f:
    data = f.read()

print('Total data length:', len(data))
print('Hex dump:', data.hex())
print()

# Parse according to 0x3408 format - similar to 0x3409 but let's analyze
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

# unk2: f32
unk2 = struct.unpack('<f', data[offset:offset+4])[0]
print(f'unk2: {unk2}')
offset += 4

print(f'After unk2, offset: 0x{offset:02X}, remaining: {len(data) - offset}')

# Let's try different parsing approaches
print('Possible format variations:')

# Option 1: 2 Vector3 values (24 bytes) then flags/bits_per_entry
print('Option 1: 2 Vector3 values + flags + bits_per_entry')
offset_temp = offset
for i in range(2):
    if offset_temp + 12 <= len(data):
        v = struct.unpack('<3f', data[offset_temp:offset_temp+12])
        print(f'  Vector3[{i}]: {v} (raw bytes: {data[offset_temp:offset_temp+12].hex()})')
        offset_temp += 12

if offset_temp + 4 <= len(data):
    flags = struct.unpack('<H', data[offset_temp:offset_temp+2])[0]
    bits_per_entry = struct.unpack('<H', data[offset_temp+2:offset_temp+4])[0]
    print(f'  flags: 0x{flags:04X}, bits_per_entry: {bits_per_entry}')
    print(f'  This would put compressed data at offset: 0x{offset_temp+4:02X}')

# Option 2: 3 Vector3 values like 0x3409
print('Option 2: 3 Vector3 values like 0x3409')
offset_temp = offset
for i in range(3):
    if offset_temp + 12 <= len(data):
        v = struct.unpack('<3f', data[offset_temp:offset_temp+12])
        print(f'  Vector3[{i}]: {v} (raw bytes: {data[offset_temp:offset_temp+12].hex()})')
        offset_temp += 12

# Option 3: flags and bits_per_entry first, then Vector3 values
print('Option 3: flags + bits_per_entry first, then Vector3 values')
offset_temp = offset
if offset_temp + 4 <= len(data):
    flags = struct.unpack('<H', data[offset_temp:offset_temp+2])[0]
    bits_per_entry = struct.unpack('<H', data[offset_temp+2:offset_temp+4])[0]
    print(f'  flags: 0x{flags:04X}, bits_per_entry: {bits_per_entry}')
    offset_temp += 4

    # Then try to parse Vector3 values
    for i in range(3):
        if offset_temp + 12 <= len(data):
            v = struct.unpack('<3f', data[offset_temp:offset_temp+12])
            print(f'  Vector3[{i}]: {v} (raw bytes: {data[offset_temp:offset_temp+12].hex()})')
            offset_temp += 12

print()
print('User provided hex data analysis:')
print('08 34 00 00 0A 00 00 00 00 00 80 3F 56 CC 35 3E 00 00 00 00 E5 D0 E2 3F 00 00 00 00 00 00 00 00 CD CC 4C 3F 00 00 00 00 01 00 01 00 01 00 02 11 FF FF 5B 22 FF 02 00 11 D7 7F D3 11 81 4B AE 46 01 00 01 00 01 00 02 11 13 10 00 00 00 00 00 00 13 10 00 00 FF 7F 00 00')
print('Magic: 0834 | FrameCount: 0A | unk1: 0000803F(1.0) | unk2: 56CC353E')
print('Vec3[0]: 00000000 E5D0E23F 00000000 (0.0, 1.772, 0.0)')
print('Vec3[1]: 00000000 CDCC4C3F 00000000 (0.0, 0.8, 0.0)')
print('Then: 01000100 01000211 FFFF5B22 ...')

# flags: u16
if offset + 2 <= len(data):
    flags = struct.unpack('<H', data[offset:offset+2])[0]
    print(f'flags: 0x{flags:04X} (at offset 0x{offset:02X})')
    offset += 2

# bits_per_entry: u16
if offset + 2 <= len(data):
    bits_per_entry = struct.unpack('<H', data[offset:offset+2])[0]
    print(f'bits_per_entry: {bits_per_entry} (at offset 0x{offset:02X})')
    offset += 2

print(f'Compressed data starts at offset: 0x{offset:02X}')
print('Remaining compressed data length:', len(data) - offset)

# Show remaining data in hex
if len(data) > offset:
    remaining = data[offset:]
    print('Remaining data (first 50 bytes):', remaining[:50].hex())
    print('Remaining data length:', len(remaining))
