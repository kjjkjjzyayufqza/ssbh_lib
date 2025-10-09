#!/usr/bin/env python3
"""
Animation Data Compression Decoder - Test Suite
Testing various decompression algorithms for 0x3409 compressed animation data
"""

import struct
import math
from typing import List, Tuple
import os


class AnimationDecoder:
    """Main decoder class for testing different decompression strategies"""
    
    def __init__(self):
        # Known ground truth values (37 frames, excluding first/middle/last)
        self.known_values = [
            1.721675,  2.773587,  3.944817,  5.231249,  6.627399,
            8.129151,  9.729649,  11.42615,  13.21042,  15.0811,
            17.029949, 19.05286,  21.144341, 23.30028,  25.51519,
            27.784969, 30.10137,  32.46302,  34.863091, 37.294689,
            39.75647,  42.240189, 44.743111, 47.258369, 49.780499,
            52.35062,  54.963249, 57.621151, 60.328419, 63.08506,
            65.896561, 68.765663,  # Frame 33
            # Frame 34 (middle) is excluded
            74.683533, 77.739143, 80.863342, 84.058853, 87.328407
            # Frame 40 (last) is excluded
        ]
        
        # Z-axis compressed data (58 bytes)
        self.z_compressed = bytes([
            0x01, 0x80, 0x29, 0x35, 0xA7, 0x0F, 0x9A, 0x0E,
            0x04, 0x60, 0xFD, 0x7F, 0x7A, 0x31, 0x03, 0x43,
            0xA6, 0x1B, 0xE4, 0x2B, 0xB6, 0x0E, 0x01, 0x20,
            0x36, 0x18, 0x8B, 0x44, 0x6A, 0x0F, 0x49, 0x34,
            0x8B, 0x08, 0xCB, 0x2A, 0x5E, 0x02, 0x43, 0x24,
            0x72, 0xFE, 0x36, 0x1E, 0x50, 0xFC, 0x2D, 0x19,
            0x0F, 0xFA, 0x01, 0x16, 0x4B, 0xF7, 0xA6, 0x13,
            0xF5, 0x11, 0xF4, 0x0E
        ])
        
        # Parameter block from the document
        self.param_block = bytes([0xFF, 0xFF, 0xE3, 0x0B, 0x58, 0x71, 0x00, 0x12])
        
        # Starting value (first frame)
        self.first_frame_value = 0.795937
        
        # Key frames for reference
        self.middle_frame_value = 71.693703  # Frame 34
        self.last_frame_value = 90.674797    # Frame 40
    
    def calculate_rmse(self, decoded: List[float]) -> float:
        """Calculate Root Mean Square Error"""
        if len(decoded) != len(self.known_values):
            return float('inf')
        
        sum_sq_error = sum((a - b) ** 2 for a, b in zip(self.known_values, decoded))
        return math.sqrt(sum_sq_error / len(self.known_values))
    
    def calculate_max_error(self, decoded: List[float]) -> float:
        """Calculate maximum absolute error"""
        if len(decoded) != len(self.known_values):
            return float('inf')
        
        return max(abs(a - b) for a, b in zip(self.known_values, decoded))
    
    def test_fixed_point_decode(self) -> None:
        """Test Method 1: 16-bit Fixed Point Decoding"""
        print("\n" + "="*70)
        print("TEST 1: 16-bit Fixed Point Decoding")
        print("="*70)
        
        scale_factors = [100.0, 1000.0, 3043.0, 10000.0, 32768.0]
        
        for scale in scale_factors:
            print(f"\n--- Scale Factor: {scale} ---")
            
            current = self.first_frame_value
            offset = 2  # Skip 01 80 header
            decoded_values = []
            
            frame_idx = 0
            while offset + 1 < len(self.z_compressed) and frame_idx < len(self.known_values):
                # Read 16-bit signed integer (little endian)
                delta_raw = struct.unpack('<h', self.z_compressed[offset:offset+2])[0]
                offset += 2
                
                delta = delta_raw / scale
                current += delta
                decoded_values.append(current)
                
                error = abs(current - self.known_values[frame_idx])
                
                if frame_idx < 5 or error > 0.5:  # Print first 5 or problematic frames
                    print(f"Frame {frame_idx+2:2d}: raw={delta_raw:6d}, delta={delta:8.3f}, "
                          f"value={current:8.3f}, real={self.known_values[frame_idx]:8.3f}, "
                          f"error={error:8.6f}")
                
                # Stop if error is too large
                if error > 2.0:
                    print(f"  [!] Large error detected, stopping...")
                    break
                
                frame_idx += 1
            
            if len(decoded_values) >= len(self.known_values):
                rmse = self.calculate_rmse(decoded_values[:len(self.known_values)])
                max_err = self.calculate_max_error(decoded_values[:len(self.known_values)])
                print(f"\n  [STATS] RMSE: {rmse:.6f}, Max Error: {max_err:.6f}")
                
                if rmse < 0.1:
                    print(f"  [SUCCESS] Scale factor {scale} produces good results!")
    
    def decode_varint_zigzag(self, data: bytes, offset: int) -> Tuple[int, int]:
        """Decode variable-length integer with ZigZag encoding"""
        result = 0
        shift = 0
        new_offset = offset
        
        while new_offset < len(data):
            byte = data[new_offset]
            new_offset += 1
            
            result |= (byte & 0x7F) << shift
            
            if (byte & 0x80) == 0:  # MSB = 0, done
                break
            
            shift += 7
        
        # ZigZag decode: 0→0, 1→-1, 2→1, 3→-2...
        signed_value = (result >> 1) ^ -(result & 1)
        
        return signed_value, new_offset
    
    def test_varint_decode(self) -> None:
        """Test Method 2: Variable-length Integer + ZigZag Decoding"""
        print("\n" + "="*70)
        print("TEST 2: Varint + ZigZag Decoding")
        print("="*70)
        
        scale_factors = [100.0, 1000.0, 10000.0]
        
        for scale in scale_factors:
            print(f"\n--- Scale Factor: {scale} ---")
            
            current = self.first_frame_value
            offset = 2  # Skip 01 80 header
            decoded_values = []
            
            frame_idx = 0
            while offset < len(self.z_compressed) and frame_idx < len(self.known_values):
                try:
                    signed_val, offset = self.decode_varint_zigzag(self.z_compressed, offset)
                    
                    delta = signed_val / scale
                    current += delta
                    decoded_values.append(current)
                    
                    error = abs(current - self.known_values[frame_idx])
                    
                    if frame_idx < 5:
                        print(f"Frame {frame_idx+2:2d}: varint={signed_val:6d}, "
                              f"value={current:8.6f}, real={self.known_values[frame_idx]:8.6f}, "
                              f"error={error:8.6f}")
                    
                    frame_idx += 1
                    
                except Exception as e:
                    print(f"  [!] Decode error: {e}")
                    break
            
            if len(decoded_values) > 0:
                rmse = self.calculate_rmse(decoded_values[:min(len(decoded_values), len(self.known_values))])
                print(f"\n  [STATS] Decoded {len(decoded_values)} frames, RMSE: {rmse:.6f}")
    
    def analyze_param_block(self) -> None:
        """Test Method 3: Parameter Block Analysis"""
        print("\n" + "="*70)
        print("TEST 3: Parameter Block Analysis")
        print("="*70)
        
        print(f"\nParameter Block: {self.param_block.hex(' ').upper()}")
        
        # Test all possible 32-bit float positions
        print("\n--- Interpreting as floats ---")
        for i in range(len(self.param_block) - 3):
            val = struct.unpack('<f', self.param_block[i:i+4])[0]
            print(f"Offset {i}: {val:15.6f}")
        
        # Test 16-bit integers
        print("\n--- Interpreting as 16-bit integers ---")
        for i in range(0, len(self.param_block), 2):
            u16 = struct.unpack('<H', self.param_block[i:i+2])[0]
            s16 = struct.unpack('<h', self.param_block[i:i+2])[0]
            print(f"Offset {i}: uint16={u16:5d} (0x{u16:04X}), int16={s16:6d}")
        
        # Check for unk2 value (28.874197)
        unk2 = 28.874197
        unk2_bytes = struct.pack('<f', unk2)
        print(f"\nunk2 (28.874197) IEEE 754 bytes: {unk2_bytes.hex(' ').upper()}")
        
        # Check scale factor hypothesis
        scale_factor = struct.unpack('<H', self.param_block[2:4])[0]
        print(f"\nPotential scale factor at offset 2: {scale_factor} (0x{scale_factor:04X})")
    
    def test_8bit_encoding(self) -> None:
        """Test Method 4: 8-bit Delta Encoding"""
        print("\n" + "="*70)
        print("TEST 4: 8-bit Delta Encoding")
        print("="*70)
        
        scale_factors = [10.0, 100.0, 255.0]
        
        for scale in scale_factors:
            print(f"\n--- Scale Factor: {scale} ---")
            
            current = self.first_frame_value
            offset = 2  # Skip 01 80 header
            decoded_values = []
            
            frame_idx = 0
            while offset < len(self.z_compressed) and frame_idx < len(self.known_values):
                # Read 8-bit signed integer
                delta_raw = struct.unpack('b', bytes([self.z_compressed[offset]]))[0]
                offset += 1
                
                delta = delta_raw / scale
                current += delta
                decoded_values.append(current)
                
                error = abs(current - self.known_values[frame_idx])
                
                if frame_idx < 5:
                    print(f"Frame {frame_idx+2:2d}: raw={delta_raw:4d}, "
                          f"value={current:8.3f}, real={self.known_values[frame_idx]:8.3f}, "
                          f"error={error:8.6f}")
                
                frame_idx += 1
            
            if len(decoded_values) > 0:
                rmse = self.calculate_rmse(decoded_values[:min(len(decoded_values), len(self.known_values))])
                print(f"\n  [STATS] RMSE: {rmse:.6f}")
    
    def hex_dump_analysis(self) -> None:
        """Analyze the raw hex data patterns"""
        print("\n" + "="*70)
        print("HEX DUMP ANALYSIS")
        print("="*70)
        
        print("\nCompressed data (58 bytes):")
        for i in range(0, len(self.z_compressed), 16):
            hex_str = ' '.join(f'{b:02X}' for b in self.z_compressed[i:i+16])
            ascii_str = ''.join(chr(b) if 32 <= b < 127 else '.' for b in self.z_compressed[i:i+16])
            print(f"{i:04X}: {hex_str:<47} | {ascii_str}")
        
        print("\n--- Header Analysis ---")
        header_word = struct.unpack('<H', self.z_compressed[0:2])[0]
        print(f"First word: 0x{header_word:04X} = {header_word} (decimal)")
        print(f"  Could be: control word, encoding type, or length marker")
        
        print("\n--- Value Distribution ---")
        values_as_16bit = []
        for i in range(2, len(self.z_compressed) - 1, 2):
            val = struct.unpack('<h', self.z_compressed[i:i+2])[0]
            values_as_16bit.append(val)
        
        print(f"Min 16-bit value: {min(values_as_16bit)}")
        print(f"Max 16-bit value: {max(values_as_16bit)}")
        print(f"Average: {sum(values_as_16bit) / len(values_as_16bit):.2f}")


def main():
    """Main test runner"""
    print("="*70)
    print("ANIMATION COMPRESSION DECODER - TEST SUITE")
    print("Testing 0x3409 compressed data with multiple algorithms")
    print("="*70)
    
    decoder = AnimationDecoder()
    
    # Run all tests
    decoder.hex_dump_analysis()
    decoder.analyze_param_block()
    decoder.test_fixed_point_decode()
    decoder.test_varint_decode()
    decoder.test_8bit_encoding()
    
    print("\n" + "="*70)
    print("TESTING COMPLETE")
    print("="*70)
    print("\n[NEXT STEPS]")
    print("  1. Review RMSE values - values < 0.1 indicate good decoding")
    print("  2. Check which scale factor produces the best results")
    print("  3. Analyze the parameter block for metadata")
    print("  4. Test with additional sample files if available")
    print()


if __name__ == "__main__":
    main()

