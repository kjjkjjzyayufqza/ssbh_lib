import struct

# Read the binary file
with open('3408_2.bin', 'rb') as f:
    data = f.read()

print('='*80)
print('ANALYZING 3408_2.bin')
print('='*80)
print(f'Total data length: {len(data)} bytes')
print(f'Hex dump: {data.hex()}')
print()

# Parse according to 0x3408 format
offset = 0

# Magic number (first 2 bytes of track data are the header)
magic = struct.unpack('<H', data[offset:offset+2])[0]
print(f'Magic: 0x{magic:04X}')
offset += 2

# Skip padding/alignment to 4 bytes
if offset % 4 != 0:
    padding = 4 - (offset % 4)
    print(f'Padding: {padding} bytes')
    offset += padding

print(f'After magic, offset: 0x{offset:02X}')

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

print(f'\nAfter header fields, offset: 0x{offset:02X}')
print('='*80)

# Two default Vector3 values (24 bytes total)
print('\nDefault Vector3 values:')
for i in range(2):
    if offset + 12 <= len(data):
        x, y, z = struct.unpack('<3f', data[offset:offset+12])
        print(f'  Vector3[{i}]: x={x:.6f}, y={y:.6f}, z={z:.6f}')
        print(f'    Raw bytes: {data[offset:offset+12].hex()}')
        offset += 12

print(f'\nAfter Vector3 values, offset: 0x{offset:02X}')
print('='*80)

# Try different interpretations of flags/bits
print('\nTrying different field interpretations:')

# Option 1: flags (u16) + bits_per_entry (u16)
offset_opt1 = offset
flags_opt1 = struct.unpack('<H', data[offset_opt1:offset_opt1+2])[0]
bits_opt1 = struct.unpack('<H', data[offset_opt1+2:offset_opt1+4])[0]
print(f'Option 1 (u16+u16): flags=0x{flags_opt1:04X}, bits_per_entry={bits_opt1}')

# Option 2: flags (u32) + bits_per_entry (u16)
offset_opt2 = offset
flags_opt2 = struct.unpack('<I', data[offset_opt2:offset_opt2+4])[0]
bits_opt2 = struct.unpack('<H', data[offset_opt2+4:offset_opt2+6])[0]
print(f'Option 2 (u32+u16): flags=0x{flags_opt2:08X}, bits_per_entry={bits_opt2}')

# Option 3: u8 + u8 + u8 + u8 + u16
offset_opt3 = offset
b0, b1, b2, b3 = struct.unpack('<4B', data[offset_opt3:offset_opt3+4])
bits_opt3 = struct.unpack('<H', data[offset_opt3+4:offset_opt3+6])[0]
print(f'Option 3 (4xu8+u16): bytes=({b0},{b1},{b2},{b3}), bits_per_entry={bits_opt3}')

# Looking at the hex data more carefully
print(f'\nBytes at offset {offset:02X}: {data[offset:offset+8].hex()}')
print(f'  Interpretation: {list(data[offset:offset+8])}')

# The bytes are: 01 00 01 00 01 01 04 12
# This looks like: 0x0001, 0x0001, 0x0101, 0x1204
# Or: 1, 0, 1, 0, 1, 1, 4, 18

# Let's assume bits_per_entry might be after some other fields
# Looking at the pattern, maybe it's: u8, u8, u8, u8, u16, u16
b0, b1, b2, b3, w1, w2 = struct.unpack('<4B2H', data[offset:offset+8])
print(f'\nMost likely interpretation (4xu8 + 2xu16):')
print(f'  bytes: ({b0}, {b1}, {b2}, {b3})')
print(f'  word1: {w1}, word2: {w2}')

# Use Option 3 interpretation
flags = b0  # or some combination
bits_per_entry = w2  # 0x1204 = 4610 in decimal
offset += 8  # Skip these fields for now

print(f'\nCompressed data starts at offset: 0x{offset:02X}')
print('='*80)

# Show remaining compressed data
if len(data) > offset:
    remaining = data[offset:]
    print(f'\nCompressed data length: {len(remaining)} bytes')
    print(f'Compressed data (hex): {remaining.hex()}')
    
    # Try to interpret as bit-packed data
    print('\n' + '='*80)
    print('BIT-PACKED DATA ANALYSIS')
    print('='*80)
    
    if bits_per_entry > 0:
        # Calculate expected bits per component
        # For Vector3, we typically divide bits_per_entry by 3
        bits_per_component = bits_per_entry // 3
        print(f'Expected bits_per_component: {bits_per_component} (bits_per_entry={bits_per_entry} / 3)')
        
        # Calculate expected total bits needed
        total_bits_needed = compressed_frame_count * bits_per_entry
        total_bytes_needed = (total_bits_needed + 7) // 8
        print(f'Expected compressed data size: {total_bytes_needed} bytes ({total_bits_needed} bits)')
        print(f'Actual compressed data size: {len(remaining)} bytes')
        
        # Try to read frames
        print(f'\nAttempting to decode {compressed_frame_count} frames:')
        print('(Using linear interpolation between two default Vector3 values)')
        print()
        
        # Convert bytes to bits
        bits = bin(int.from_bytes(remaining, byteorder='little'))[2:].zfill(len(remaining) * 8)
        bit_offset = 0
        
        default_v0 = struct.unpack('<3f', data[16:28])  # First Vector3
        default_v1 = struct.unpack('<3f', data[28:40])  # Second Vector3
        
        print(f'Default V0: ({default_v0[0]:.6f}, {default_v0[1]:.6f}, {default_v0[2]:.6f})')
        print(f'Default V1: ({default_v1[0]:.6f}, {default_v1[1]:.6f}, {default_v1[2]:.6f})')
        print()
        
        if bits_per_component > 0:
            max_index = (1 << bits_per_component) - 1
            print(f'Max index for normalization: {max_index}')
            print()
            
            for frame_idx in range(min(compressed_frame_count, 10)):  # Limit to first 10 frames
                if bit_offset + bits_per_entry <= len(bits):
                    # Read indices for x, y, z
                    indices = []
                    for component in range(3):
                        if bit_offset + bits_per_component <= len(bits):
                            # Read bits in LSB order
                            index_bits = bits[bit_offset:bit_offset + bits_per_component]
                            if len(index_bits) > 0:
                                index = int(index_bits[::-1], 2)  # Reverse for LSB-first
                                indices.append(index)
                                bit_offset += bits_per_component
                    
                    if len(indices) == 3:
                        # Normalize indices to [0, 1]
                        t_values = [idx / max_index if max_index > 0 else 0.0 for idx in indices]
                        
                        # Linear interpolation
                        x = default_v0[0] + (default_v1[0] - default_v0[0]) * t_values[0]
                        y = default_v0[1] + (default_v1[1] - default_v0[1]) * t_values[1]
                        z = default_v0[2] + (default_v1[2] - default_v0[2]) * t_values[2]
                        
                        print(f'Frame {frame_idx}: indices=({indices[0]:3d}, {indices[1]:3d}, {indices[2]:3d}) '
                              f't=({t_values[0]:.4f}, {t_values[1]:.4f}, {t_values[2]:.4f}) '
                              f'→ ({x:.6f}, {y:.6f}, {z:.6f})')
        else:
            print('bits_per_component is 0, cannot decode frames')

print('\n' + '='*80)
print('ANALYSIS COMPLETE')
print('='*80)

