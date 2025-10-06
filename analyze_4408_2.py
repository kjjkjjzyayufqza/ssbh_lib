import struct

# Read the binary file
with open('4408_2.bin', 'rb') as f:
    data = f.read()

print('Total data length:', len(data))
print('Hex dump:', data.hex())
print()

# Parse according to 0x4408 format
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

# Two default Vector4 values (16 bytes each)
print('Default values:')
for i in range(2):
    v = struct.unpack('<4f', data[offset:offset+16])
    print(f'  Vector4[{i}]: {v}')
    offset += 16

# flags: u16
flags = struct.unpack('<H', data[offset:offset+2])[0]
print(f'flags: 0x{flags:04X} (at offset 0x{offset:02X})')
offset += 2

# bits_per_entry: u16
bits_per_entry = struct.unpack('<H', data[offset:offset+2])[0]
print(f'bits_per_entry: {bits_per_entry} (at offset 0x{offset:02X})')
offset += 2

print(f'Compressed data starts at offset: 0x{offset:02X}')
print('Remaining compressed data length:', len(data) - offset)

# Show remaining data in hex
if len(data) > offset:
    print('Remaining data (first 50 bytes):', data[offset:offset+50].hex())
