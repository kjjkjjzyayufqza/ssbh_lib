import struct

# The data from terminal output
data_array = [8, 52, 0, 0, 20, 0, 0, 0, 0, 0, 128, 63, 242, 178, 203, 62, 0, 0, 0, 0, 48, 100, 193, 63, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64, 64, 0, 0, 0, 0, 1, 0, 1, 0, 1, 1, 4, 18, 0, 0, 0, 0, 255, 255, 202, 30, 94, 5, 0, 18, 9, 129, 29, 22, 239, 127, 224, 79, 234, 47, 246, 24, 3, 19, 25, 244, 39, 224, 45, 213, 1, 0, 1, 0, 1, 1, 4, 18, 0, 0, 0, 0]

data = bytes(data_array)

print('='*80)
print('FINAL ANALYSIS WITH TERMINAL DATA')
print('='*80)
print(f'Total length: {len(data)} bytes\n')

# Show hex dump
print('HEX DUMP:')
for i in range(0, len(data), 16):
    hex_part = ' '.join(f'{b:02X}' for b in data[i:i+16])
    print(f'{i:04X}: {hex_part}')
print()

# Parse header
offset = 0
header = struct.unpack('<I', data[offset:offset+4])[0]
print(f'0x{offset:04X}: header = 0x{header:08X}')
offset += 4

compressed_frame_count = struct.unpack('<I', data[offset:offset+4])[0]
print(f'0x{offset:04X}: compressed_frame_count = {compressed_frame_count}')
offset += 4

unk1 = struct.unpack('<f', data[offset:offset+4])[0]
print(f'0x{offset:04X}: unk1 = {unk1}')
offset += 4

unk2 = struct.unpack('<f', data[offset:offset+4])[0]
print(f'0x{offset:04X}: unk2 = {unk2}')
offset += 4

default_v0 = struct.unpack('<3f', data[offset:offset+12])
print(f'0x{offset:04X}: default_v0 = ({default_v0[0]}, {default_v0[1]}, {default_v0[2]})')
offset += 12

default_v1 = struct.unpack('<3f', data[offset:offset+12])
print(f'0x{offset:04X}: default_v1 = ({default_v1[0]}, {default_v1[1]}, {default_v1[2]})')
offset += 12

flags = struct.unpack('<H', data[offset:offset+2])[0]
print(f'0x{offset:04X}: flags = 0x{flags:04X}')
offset += 2

bits_per_entry = struct.unpack('<H', data[offset:offset+2])[0]
print(f'0x{offset:04X}: bits_per_entry = {bits_per_entry}')
offset += 2

print(f'\n0x{offset:04X}: REMAINING DATA ({len(data)-offset} bytes)')
remaining = data[offset:]

# Pattern analysis
print('\n' + '='*80)
print('PATTERN ANALYSIS')
print('='*80)

# First 8 bytes after bits_per_entry
pattern1 = remaining[0:8]
print(f'Bytes at 0x{offset:04X}: {pattern1.hex()} = {list(pattern1)}')

# Last 12 bytes
pattern2 = remaining[-12:]
print(f'Last 12 bytes:     {pattern2.hex()} = {list(pattern2)}')

# Check if pattern repeats
if pattern1 == pattern2[4:]:
    print('\n*** PATTERN MATCH: First 8 bytes == Last 8 bytes ***')

# Middle section (actual compressed data)
middle_start = 8
middle_end = len(remaining) - 12
middle_data = remaining[middle_start:middle_end]
print(f'\nMiddle data (0x{offset+middle_start:04X} to 0x{offset+middle_end:04X}): {len(middle_data)} bytes')
print(f'Hex: {middle_data.hex()}')

# Theory: Maybe the format includes BOTH pattern before AND after compressed data?
# Let's try decoding just the middle section
print('\n' + '='*80)
print('HYPOTHESIS: Compressed data is ONLY the middle section')
print('='*80)

bits_per_component = 8  # For bits_per_entry=1
print(f'bits_per_component: {bits_per_component}')
print(f'Middle data size: {len(middle_data)} bytes = {len(middle_data)*8} bits')
print(f'Can decode: {(len(middle_data)*8) // (bits_per_component*3)} complete frames')

# Try alternative: What if the ENTIRE remaining section (including patterns) is compressed data?
print('\n' + '='*80)
print('ALTERNATIVE: Use ALL remaining data as compressed')
print('='*80)
print(f'All remaining: {len(remaining)} bytes = {len(remaining)*8} bits')
print(f'Can decode: {(len(remaining)*8) // (bits_per_component*3)} complete frames')

# Decode with all remaining data
all_bits = []
for byte in remaining:
    for i in range(8):
        all_bits.append((byte >> i) & 1)

max_index = 255
bit_offset = 0
frames = []

print(f'\nDecoding {compressed_frame_count} frames:')
for frame_idx in range(compressed_frame_count):
    if bit_offset + 24 > len(all_bits):
        print(f'Frame {frame_idx}: NOT ENOUGH BITS (need 24, have {len(all_bits)-bit_offset})')
        break
    
    indices = []
    for _ in range(3):
        index = 0
        for bit_idx in range(8):
            if bit_offset < len(all_bits):
                index |= all_bits[bit_offset] << bit_idx
                bit_offset += 1
        indices.append(index)
    
    t_values = [idx / max_index for idx in indices]
    x = default_v0[0] + (default_v1[0] - default_v0[0]) * t_values[0]
    y = default_v0[1] + (default_v1[1] - default_v0[1]) * t_values[1]
    z = default_v0[2] + (default_v1[2] - default_v0[2]) * t_values[2]
    
    frames.append((x, y, z))
    
    if frame_idx < 8 or frame_idx >= compressed_frame_count - 2:
        print(f'Frame {frame_idx:2d}: idx=({indices[0]:3d},{indices[1]:3d},{indices[2]:3d}) t=({t_values[0]:.4f},{t_values[1]:.4f},{t_values[2]:.4f}) → ({x:.6f},{y:.6f},{z:.6f})')
    elif frame_idx == 8:
        print('...')

print(f'\nTotal frames successfully decoded: {len(frames)}/{compressed_frame_count}')

if len(frames) < compressed_frame_count:
    print(f'\n*** WARNING: Could only decode {len(frames)} frames, expected {compressed_frame_count} ***')
    print(f'*** This suggests bits_per_entry={bits_per_entry} might trigger fallback to linear interpolation ***')

