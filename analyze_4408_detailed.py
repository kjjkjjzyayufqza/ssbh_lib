import struct

# Read the binary file
with open('4408.bin', 'rb') as f:
    data = f.read()

print('='*60)
print('Detailed 4408.bin Analysis')
print('='*60)
print(f'Total data length: {len(data)} bytes')
print()

# Parse byte by byte
offset = 0

# Magic number
magic = struct.unpack('<I', data[offset:offset+4])[0]
print(f'Offset 0x{offset:02X}: Magic = 0x{magic:04X}')
offset += 4

# compressed_frame_count
compressed_frame_count = struct.unpack('<I', data[offset:offset+4])[0]
print(f'Offset 0x{offset:02X}: compressed_frame_count = {compressed_frame_count}')
offset += 4

# unk1
unk1 = struct.unpack('<f', data[offset:offset+4])[0]
print(f'Offset 0x{offset:02X}: unk1 (f32) = {unk1}')
offset += 4

# unk2
unk2 = struct.unpack('<f', data[offset:offset+4])[0]
print(f'Offset 0x{offset:02X}: unk2 (f32) = {unk2}')
offset += 4

# Two Vector4 default values
print()
print('Default Vector4 values:')
for i in range(2):
    v = struct.unpack('<4f', data[offset:offset+16])
    print(f'Offset 0x{offset:02X}: Vector4[{i}] = ({v[0]:.10f}, {v[1]:.10f}, {v[2]:.10f}, {v[3]:.10f})')
    offset += 16

print()
print(f'Offset 0x{offset:02X}: Next bytes (hex) = {data[offset:offset+8].hex()}')

# Try different interpretations:
print()
print('Interpretation 1: flags (u16) + bits_per_entry (u16)')
flags = struct.unpack('<H', data[offset:offset+2])[0]
bits_per_entry = struct.unpack('<H', data[offset+2:offset+4])[0]
print(f'  flags = 0x{flags:04X}')
print(f'  bits_per_entry = {bits_per_entry} (0x{bits_per_entry:04X})')

print()
print('Interpretation 2: as single u32')
val_u32 = struct.unpack('<I', data[offset:offset+4])[0]
print(f'  u32 = 0x{val_u32:08X}')

print()
print('Interpretation 3: Check if there might be additional fields')
# Check next 16 bytes
print(f'Next 16 bytes after current position:')
for i in range(4):
    pos = offset + i*4
    if pos + 4 <= len(data):
        val = struct.unpack('<I', data[pos:pos+4])[0]
        print(f'  Offset 0x{pos:02X}: 0x{val:08X} (as u32) or {data[pos:pos+4].hex()}')

# Calculate expected compressed data size
print()
print('Compressed data analysis:')
offset_after_header = offset + 4  # After flags + bits_per_entry
compressed_data_size = len(data) - offset_after_header
print(f'Compressed data starts at offset: 0x{offset_after_header:02X}')
print(f'Compressed data size: {compressed_data_size} bytes')
print(f'First 32 bytes of compressed data: {data[offset_after_header:offset_after_header+32].hex()}')

# Try to validate bits_per_entry
print()
print('Validation:')
if bits_per_entry > 0:
    bits_total = bits_per_entry * compressed_frame_count
    bytes_needed = (bits_total + 7) // 8
    print(f'If bits_per_entry = {bits_per_entry}:')
    print(f'  Total bits needed = {bits_per_entry} * {compressed_frame_count} = {bits_total}')
    print(f'  Bytes needed = {bytes_needed}')
    print(f'  Actual compressed data size = {compressed_data_size}')
    print(f'  Match: {bytes_needed == compressed_data_size}')

