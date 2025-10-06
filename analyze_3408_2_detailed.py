import struct

# Read the binary file
with open('3408_2.bin', 'rb') as f:
    data = f.read()

print('='*80)
print('DETAILED ANALYSIS OF 3408_2.bin')
print('='*80)
print(f'Total data length: {len(data)} bytes\n')

# Show full hex dump with offsets
print('HEX DUMP:')
for i in range(0, len(data), 16):
    hex_part = ' '.join(f'{b:02X}' for b in data[i:i+16])
    ascii_part = ''.join(chr(b) if 32 <= b < 127 else '.' for b in data[i:i+16])
    print(f'{i:04X}: {hex_part:<48} {ascii_part}')
print()

# Parse according to 0x3408 format
offset = 0

# Magic number (note: this is actually part of the data array from Rust)
magic = struct.unpack('<H', data[offset:offset+2])[0]
print(f'Offset 0x{offset:04X}: Magic = 0x{magic:04X}')
offset += 2

# Padding to 4-byte alignment
padding_bytes = (4 - (offset % 4)) % 4
if padding_bytes > 0:
    print(f'Offset 0x{offset:04X}: Padding = {padding_bytes} bytes')
    offset += padding_bytes

# compressed_frame_count: u32
compressed_frame_count = struct.unpack('<I', data[offset:offset+4])[0]
print(f'Offset 0x{offset:04X}: compressed_frame_count = {compressed_frame_count}')
offset += 4

# unk1: f32
unk1 = struct.unpack('<f', data[offset:offset+4])[0]
print(f'Offset 0x{offset:04X}: unk1 = {unk1:.8f}')
offset += 4

# unk2: f32
unk2 = struct.unpack('<f', data[offset:offset+4])[0]
print(f'Offset 0x{offset:04X}: unk2 = {unk2:.8f}')
offset += 4

print()

# Two default Vector3 values
print('DEFAULT VECTOR3 VALUES:')
default_vectors = []
for i in range(2):
    x, y, z = struct.unpack('<3f', data[offset:offset+12])
    default_vectors.append((x, y, z))
    print(f'Offset 0x{offset:04X}: Vector3[{i}] = ({x:.8f}, {y:.8f}, {z:.8f})')
    offset += 12

print()

# flags: u16
flags = struct.unpack('<H', data[offset:offset+2])[0]
print(f'Offset 0x{offset:04X}: flags = 0x{flags:04X} ({flags})')
offset += 2

# bits_per_entry: u16
bits_per_entry = struct.unpack('<H', data[offset:offset+2])[0]
print(f'Offset 0x{offset:04X}: bits_per_entry = {bits_per_entry}')
offset += 2

# Check what's next
print(f'\nOffset 0x{offset:04X}: Next bytes = {data[offset:offset+8].hex()}')
print(f'Interpretation: {list(data[offset:offset+8])}')

# It seems there might be additional fields before compressed data
# Let's check if there's a pattern
next_dword = struct.unpack('<I', data[offset:offset+4])[0]
print(f'  As u32: {next_dword} (0x{next_dword:08X})')

# Check if this could be a byte count or similar
if next_dword == 0x12040101:
    print(f'  Pattern suggests: possible multi-byte structure')
    # Try as 4 separate bytes
    b0, b1, b2, b3 = struct.unpack('<4B', data[offset:offset+4])
    print(f'  As 4 x u8: ({b0}, {b1}, {b2}, {b3})')
    offset += 4

# Another u32?
next_dword2 = struct.unpack('<I', data[offset:offset+4])[0]
print(f'Offset 0x{offset:04X}: Next u32 = {next_dword2} (0x{next_dword2:08X})')
offset += 4

# NOW compressed data should start
print(f'\nOffset 0x{offset:04X}: COMPRESSED DATA BEGINS')
compressed_data = data[offset:]
print(f'Compressed data length: {len(compressed_data)} bytes')
print(f'Compressed data (hex): {compressed_data.hex()}')

print('\n' + '='*80)
print('ATTEMPTING DECOMPRESSION')
print('='*80)

if bits_per_entry > 0 and bits_per_entry <= 32:
    bits_per_component = bits_per_entry  # For testing, assume this is already per-component
    
    # But wait - if bits_per_entry is 1, that's suspicious
    # Let's also try interpreting it as total bits for all 3 components
    print(f'\nInterpretation 1: bits_per_entry = {bits_per_entry} (total for Vector3)')
    print(f'  bits_per_component = {bits_per_entry // 3 if bits_per_entry >= 3 else 0}')
    
    print(f'\nInterpretation 2: bits_per_entry = {bits_per_entry} (per component)')
    print(f'  total_bits_per_frame = {bits_per_entry * 3}')
    
    # Calculate expected size
    total_bits_needed = compressed_frame_count * bits_per_entry * 3
    total_bytes_needed = (total_bits_needed + 7) // 8
    print(f'\nExpected compressed data size (if bits_per_entry is per component):')
    print(f'  {total_bytes_needed} bytes ({total_bits_needed} bits)')
    print(f'Actual: {len(compressed_data)} bytes')
    
    # Check if compressed data might be in a different format
    # Maybe it's NOT bit-packed at all for such low bit counts?
    if bits_per_entry == 1:
        print('\n*** bits_per_entry=1 is unusual ***')
        print('This might indicate a special format or that compression is not used.')
        print('Checking if data could be uncompressed floats instead...')
        
        # Check if we have enough bytes for uncompressed Vector3 data
        uncompressed_size = compressed_frame_count * 12  # 3 floats * 4 bytes
        print(f'Uncompressed Vector3 size would be: {uncompressed_size} bytes')
        print(f'Available: {len(compressed_data)} bytes')
        
        if len(compressed_data) >= 12:
            print('\nFirst few values if interpreted as raw floats:')
            for i in range(min(3, len(compressed_data) // 12)):
                offset_f = i * 12
                if offset_f + 12 <= len(compressed_data):
                    x, y, z = struct.unpack('<3f', compressed_data[offset_f:offset_f+12])
                    print(f'  Frame {i}: ({x:.8f}, {y:.8f}, {z:.8f})')

print('\n' + '='*80)
print('ANALYSIS COMPLETE')
print('='*80)

