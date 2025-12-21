// -----------------------------------------------------------------------------
// Anim v1.2 (nuanmb) encoding helpers for EXVS2 compatibility.
// This module provides encoders for compressed animation formats that match
// the game's expectations for playback.
// -----------------------------------------------------------------------------

use crate::anim_data::{error, Vector3, Vector4};
use crate::anim_data::nuanmb_v12::{kernel_row, G_CURVE_SHORT4_SCALE};

/// A unified input type for encoding multi-frame v1.2 curve buffers.
///
/// This exists to ensure all multi-frame Transform properties route through the
/// same compression entrypoint, even if the underlying format differs (0x3409 vs 0x4409).
#[derive(Debug, Clone, Copy)]
pub enum MultiframeCurve<'a> {
    Vector3(&'a [Vector3]),
    Quaternion(&'a [Vector4]),
}

// -------------------------- Helper Functions -------------------------------
#[allow(dead_code)]
#[inline]
fn align_up(x: usize, align: usize) -> usize {
    if align == 0 {
        return x;
    }
    let m = x % align;
    if m == 0 {
        x
    } else {
        x + (align - m)
    }
}

fn compute_block_count(key_count: usize) -> usize {
    if key_count <= 1 {
        1
    } else {
        (key_count - 1) / 33 + 1
    }
}

fn compute_block_len(key_count: usize, block_idx: usize) -> usize {
    if key_count <= 1 {
        return 1;
    }
    let last_block = (key_count - 1) / 33;
    if block_idx == last_block {
        key_count - 33 * block_idx - 1
    } else {
        33
    }
}

fn normalize_quaternions(values: &[Vector4]) -> Vec<Vector4> {
    // Normalize all quaternions and enforce sign continuity.
    // Many runtimes interpolate quaternions directly, so hemisphere flips can cause visible pops.
    let mut out: Vec<Vector4> = values
        .iter()
        .map(|q| {
            let n2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
            if n2 > 1.0e-12 {
                let inv = n2.sqrt().recip();
                Vector4 {
                    x: q.x * inv,
                    y: q.y * inv,
                    z: q.z * inv,
                    w: q.w * inv,
                }
            } else {
                Vector4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                }
            }
        })
        .collect();

    for i in 1..out.len() {
        let prev = out[i - 1];
        let curr = out[i];
        let dot = prev.x * curr.x + prev.y * curr.y + prev.z * curr.z + prev.w * curr.w;
        if dot < 0.0 {
            out[i] = Vector4 {
                x: -curr.x,
                y: -curr.y,
                z: -curr.z,
                w: -curr.w,
            };
        }
    }

    out
}

// ------------------------ Residual Encoder (kernel model) -------------------
//
// The v1.2 0x3409/0x4409 residual family uses:
// - A DCT-like orthonormal kernel matrix (v18 * 4 dimension, with v18 in 1..=8).
// - A header-driven weight schedule (8 slots) derived from word0/word1/byte4/byte7.
// - Quantized coefficient entries (i16/i8/nibble pairs).
//
// The runtime appears to be strict about layout variants. To maximize compatibility and keep
// the implementation deterministic, we use a fixed, known-good header shape:
// - v18 = 8
// - v14 = 8 (8 i16 coefficient entries; no i8, no nibble payload)
// - v15 = 0, v12 = 0, v13 = 0
// - byte7 = 0x12 (w16=1, w17=2) as observed in shipped assets
//
// For components that are effectively zero, we emit a zero-residual header with:
// - v18 = 8 via v12=8
// - no payload bytes (stream size = 8)
const RESIDUAL_BYTE7_W12: u8 = 0x12; // w16=1, w17=2
const RESIDUAL_V18: usize = 8;
const RESIDUAL_DIM: usize = RESIDUAL_V18 * 4; // 32

fn f32_roundtrip(x: f32) -> f32 {
    // Force float32 rounding behavior for determinism (no extended precision).
    f32::from_le_bytes(x.to_le_bytes())
}

fn residual_weights_fixed(base_scale: f32) -> [f32; 8] {
    // Pick header parameters that yield stable, non-zero weights:
    // - word0 = 65535 -> base_amp ~= base_scale
    // - word1 = 65535 -> slope ~= base_amp
    // - byte4 = 255   -> scaled_slope == slope
    //
    // This makes all 8 weight slots effectively equal, which avoids numerical edge cases.
    let word0 = 65535.0f32;
    let word1 = 65535.0f32;
    let byte4 = 255.0f32;
    let base_amp = f32_roundtrip(word0 * (1.0 / 65536.0) * base_scale);
    let slope = f32_roundtrip(word1 * (1.0 / 65536.0) * base_amp);
    let scaled_slope = f32_roundtrip(slope * (byte4 / 255.0));
    [base_amp, slope, slope, scaled_slope, scaled_slope, scaled_slope, scaled_slope, scaled_slope]
}

fn encode_residual_component_kernel_fixed(stream: &mut Vec<u8>, residuals: &[f32], base_scale: f32) {
    // If all residuals are very small, emit the zero-residual header observed in-game.
    let mut max_abs = 0.0f32;
    for &r in residuals {
        max_abs = max_abs.max(r.abs());
    }

    if max_abs <= 1.0e-12 {
        // Header bytes: word0=1, word1=1, byte4=1, byte5=0, byte6=0x08 (v12=8), byte7=0x12
        stream.extend_from_slice(&1u16.to_le_bytes());
        stream.extend_from_slice(&1u16.to_le_bytes());
        stream.push(1u8);
        stream.push(0u8);
        stream.push(0x08u8);
        stream.push(RESIDUAL_BYTE7_W12);
        return;
    }

    // Fixed header: v14=8 i16 entries, v15=0, v12=0, v13=0, byte7=0x12.
    // Header parameters chosen to keep weights non-zero and stable.
    let word0_u16: u16 = 0xFFFF;
    let word1_u16: u16 = 0xFFFF;
    let byte4_u8: u8 = 0xFF;
    let byte5: u8 = 0x80; // v14=8, v15=0
    let byte6: u8 = 0x00; // v12=0, v13=0

    stream.extend_from_slice(&word0_u16.to_le_bytes());
    stream.extend_from_slice(&word1_u16.to_le_bytes());
    stream.push(byte4_u8);
    stream.push(byte5);
    stream.push(byte6);
    stream.push(RESIDUAL_BYTE7_W12);

    // Build y (length 32): first (block_len-1) residual samples, rest zeros.
    let mut y = [0.0f32; RESIDUAL_DIM];
    for (i, &r) in residuals.iter().take(RESIDUAL_DIM).enumerate() {
        y[i] = f32_roundtrip(r);
    }

    // x = K^T y using float32 multiply-add accumulation.
    let mut x = [0.0f32; RESIDUAL_DIM];
    for col in 0..RESIDUAL_DIM {
        let mut s = 0.0f32;
        for row in 0..RESIDUAL_DIM {
            let basis_row = kernel_row(RESIDUAL_V18, row);
            let k = basis_row[col];
            s = f32_roundtrip(s + f32_roundtrip(f32_roundtrip(k) * f32_roundtrip(y[row])));
        }
        x[col] = f32_roundtrip(s);
    }

    let w = residual_weights_fixed(base_scale);
    // v14=8 => 8 entries, each is a vec4.
    for entry_idx in 0..RESIDUAL_V18 {
        let weight = w[entry_idx % 8];
        let inv = if weight.abs() > 1.0e-30 { weight.recip() } else { 0.0 };
        let v = [
            f32_roundtrip(x[4 * entry_idx + 0] * inv),
            f32_roundtrip(x[4 * entry_idx + 1] * inv),
            f32_roundtrip(x[4 * entry_idx + 2] * inv),
            f32_roundtrip(x[4 * entry_idx + 3] * inv),
        ];

        // Quantize to i16 using the same scale as the decoder.
        for &c in &v {
            let q = (c / G_CURVE_SHORT4_SCALE).round().clamp(-32768.0, 32767.0) as i16;
            stream.extend_from_slice(&q.to_le_bytes());
        }
    }
}

/// Encode any multi-frame v1.2 curve buffer using a single entrypoint.
///
/// - Vector3 curves are encoded as `0x3409`.
/// - Quaternion curves are encoded as `0x4409`.
pub fn encode_multiframe_curve(curve: MultiframeCurve<'_>) -> Result<Vec<u8>, error::Error> {
    match curve {
        MultiframeCurve::Vector3(v) => encode_vector3_3409(v),
        MultiframeCurve::Quaternion(q) => {
            let normalized = normalize_quaternions(q);
            encode_rotate_4409(&normalized)
        }
    }
}

// ========================== 0x3409 Vector3 Encoder =======================

/// Encode Vector3 data to 0x3409 compressed format.
/// 
/// This encoder produces compressed Vector3 data (translation/scale) compatible with EXVS2.
/// It uses a simplified but effective approach:
/// - 33-key blocks with endpoint + residual encoding
/// - Endpoints fitted using block first/last values
/// - Residuals encoded with minimal DCT coefficients
/// 
/// # Parameters
/// - `values`: The Vector3 values to encode (one per frame)
/// 
/// # Returns
/// Compressed buffer with 0x3409 header
/// 
/// # Encoding Strategy
/// - Block size: 33 keys per block (matches decoder)
/// - Endpoints: Use first and last value of each block
/// - Residuals: DCT-based encoding for accurate reconstruction
/// - comp_bits: Fixed at 3 (X, Y, Z components)
/// 
/// # Notes
/// This is a simplified encoder that prioritizes game compatibility over
/// optimal compression ratio.
/// 
/// # Usage
/// Can be used for both translation and scale data, as they share the same format.
pub fn encode_vector3_3409(values: &[Vector3]) -> Result<Vec<u8>, error::Error> {
    if values.is_empty() {
        return Err(error::Error::InvalidData);
    }

    let key_count = values.len();
    let blocks = compute_block_count(key_count);
    let endpoint_count = blocks + 1;

    // Determine which axes actually vary to match observed in-game bits variants.
    let eps = 1.0e-6f32;
    let x0 = values[0].x;
    let y0 = values[0].y;
    let z0 = values[0].z;
    let mut vary_mask: u8 = 0;
    if values.iter().any(|v| (v.x - x0).abs() > eps) {
        vary_mask |= 0x1;
    }
    if values.iter().any(|v| (v.y - y0).abs() > eps) {
        vary_mask |= 0x2;
    }
    if values.iter().any(|v| (v.z - z0).abs() > eps) {
        vary_mask |= 0x4;
    }

    // Step 1: Fit endpoints (use first/last value of each block)
    let mut endpoints = Vec::with_capacity(endpoint_count);
    for block_idx in 0..blocks {
        let start_key = block_idx * 33;
        endpoints.push(values[start_key]);
    }
    // Add final endpoint
    endpoints.push(values[key_count - 1]);

    // Step 2: Compute residuals for all keys
    let mut residuals = Vec::with_capacity(key_count);
    let mut max_residual = 0.0f32;
    
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let block_len = compute_block_len(key_count, block_idx).max(1);
        let t = local as f32 / block_len as f32;
        
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let predicted = Vector3 {
            x: e0.x + (e1.x - e0.x) * t,
            y: e0.y + (e1.y - e0.y) * t,
            z: e0.z + (e1.z - e0.z) * t,
        };
        
        let actual = values[key_idx];
        let residual = Vector3 {
            x: actual.x - predicted.x,
            y: actual.y - predicted.y,
            z: actual.z - predicted.z,
        };
        
        max_residual = max_residual.max(residual.x.abs())
            .max(residual.y.abs())
            .max(residual.z.abs());
        
        residuals.push(residual);
    }

    // Step 3: Compute base_scale (max absolute residual)
    let base_scale = if max_residual > 1e-6 { max_residual } else { 1.0 };

    // Step 4: Encode residuals using the kernel residual model (game-compatible family).
    let (residual_stream, q_counts) = encode_residuals(&residuals, key_count, base_scale);

    // Step 5: Pack the buffer
    let mut data = Vec::new();
    
    // Header
    data.extend_from_slice(&0x3409u32.to_le_bytes());
    data.extend_from_slice(&(key_count as u32).to_le_bytes());
    data.extend_from_slice(&1.0f32.to_le_bytes()); // unk1
    data.extend_from_slice(&base_scale.to_le_bytes());
    // Observed in shipped assets:
    // - flags often match the number of 33-key blocks for 0x3409 Vector3 curves.
    // - endpoint_base can be 0x14 (flags=2, 2 blocks) or 0x18 (flags>=3).
    let flags_u16 = u16::try_from(blocks).unwrap_or(u16::MAX);
    data.extend_from_slice(&flags_u16.to_le_bytes());

    // bits appears to encode an axis activity variant in shipped assets.
    // Known mappings from real files:
    // - varyMask=0x7 (XYZ) -> bits=0x0020
    // - varyMask=0x5 (XZ)  -> bits=0x0022
    // - varyMask=0x3 (XY)  -> bits=0x001A
    // For unknown masks, mark XYZ as active (0x0020). The encoder always writes 3 components.
    let bits_u16: u16 = match vary_mask {
        0x7 => 0x0020,
        0x5 => 0x0022,
        0x3 => 0x001A,
        _ => 0x0020,
    };
    data.extend_from_slice(&bits_u16.to_le_bytes());

    let endpoint_base = if blocks >= 3 { 0x18usize } else { 0x14usize };
    if endpoint_base == 0x18 {
        // For endpoint_base=0x18, many files store a u32 at 0x14 that matches the prefix sum
        // of q_counts (in 32-bit words) for all blocks before the last one.
        let prefix_words_before_last: u32 = q_counts
            .iter()
            .take(blocks.saturating_sub(1))
            .copied()
            .sum::<usize>() as u32;
        data.extend_from_slice(&prefix_words_before_last.to_le_bytes());
    }

    // Endpoints (Vector3 f32 format, 12 bytes each).
    for ep in &endpoints {
        data.extend_from_slice(&ep.x.to_le_bytes());
        data.extend_from_slice(&ep.y.to_le_bytes());
        data.extend_from_slice(&ep.z.to_le_bytes());
    }

    // Residual stream (already 4-byte aligned)
    data.extend_from_slice(&residual_stream);

    Ok(data)
}

/// Encode residuals for 0x3409 format.
/// 
/// This encodes actual residual data for each block using DCT-like coefficients.
/// The residuals represent the difference between actual values and linear interpolation
/// between endpoints.
/// 
/// # Parameters
/// - `residuals`: Pre-computed residual values (actual - predicted)
/// - `key_count`: Total number of keys in the animation
/// - `base_scale`: Scale factor for residual quantization
/// 
/// # Returns
/// Encoded residual stream (4-byte aligned)
fn encode_residuals(
    residuals: &[Vector3],
    key_count: usize,
    base_scale: f32,
) -> (Vec<u8>, Vec<usize>) {
    let blocks = compute_block_count(key_count);
    let mut stream = Vec::new();
    let mut q_counts: Vec<usize> = Vec::with_capacity(blocks);

    // For each block, encode residuals for X, Y, Z components
    for block_idx in 0..blocks {
        let block_len = compute_block_len(key_count, block_idx);
        if block_len <= 1 {
            q_counts.push(0);
            continue; // No residuals needed for single-key blocks
        }
        let start_stream_len = stream.len();

        // The block contains keys at indices [block_idx*33, block_idx*33 + block_len),
        // where block_len is 33 for most blocks and can be 0/short for the last block.
        // Residuals are stored for local indices 1..(block_len-1), i.e. key indices:
        // (block_idx*33 + 1) .. (block_idx*33 + block_len - 1), inclusive.
        let start_key = block_idx * 33 + 1; // Skip first key (endpoint).
        let end_key = (block_idx * 33 + block_len).min(key_count); // Exclusive.
        
        // Extract residuals for this block
        let mut block_residuals_x = Vec::new();
        let mut block_residuals_y = Vec::new();
        let mut block_residuals_z = Vec::new();
        
        for key_idx in start_key..end_key {
            block_residuals_x.push(residuals[key_idx].x);
            block_residuals_y.push(residuals[key_idx].y);
            block_residuals_z.push(residuals[key_idx].z);
        }

        // Encode each component
        encode_residual_component_kernel_fixed(&mut stream, &block_residuals_x, base_scale);
        encode_residual_component_kernel_fixed(&mut stream, &block_residuals_y, base_scale);
        encode_residual_component_kernel_fixed(&mut stream, &block_residuals_z, base_scale);

        let delta_bytes = stream.len().saturating_sub(start_stream_len);
        q_counts.push(delta_bytes / 4);
    }

    // Ensure at least 4 bytes in the stream for decoder inference to work
    // The decoder's inference logic requires residual_off < bytes.len()
    if stream.is_empty() {
        stream.extend_from_slice(&[0u8; 4]);
    }

    // Ensure 4-byte alignment
    while stream.len() % 4 != 0 {
        stream.push(0);
    }

    (stream, q_counts)
}

/// Encode a residual component using simplified DCT-like coefficients.
/// 
/// This creates a residual encoding that matches the game's decoder expectations.
/// Uses a minimal coefficient representation that balances compression and accuracy.
/// 
/// # Strategy
/// - Compute DCT-like transform coefficients from residual samples
/// - Quantize coefficients based on base_scale
/// - Pack into the expected binary format with proper headers
/// 
/// # Layout
/// - word0 (u16): base amplitude (max absolute residual, scaled)
/// - word1 (u16): slope (linear trend, scaled)
/// - byte4 (u8): amplitude multiplier
/// - byte5 (u8): v14 (16-bit coeff count), v15 (8-bit coeff count)
/// - byte6 (u8): v12, v13 (additional coefficient flags)
/// - byte7 (u8): v16, v17 (weight distribution flags)
/// - coefficient data: quantized DCT coefficients
// NOTE: The previous simplified DCT encoder was removed in favor of the
// kernel residual model encoder above for better runtime compatibility.

// ========================== Convenience Aliases ============================

/// Encode translation data to 0x3409 compressed format.
/// 
/// This is an alias for `encode_vector3_3409` for clarity when encoding translation.
pub fn encode_translate_3409(values: &[Vector3]) -> Result<Vec<u8>, error::Error> {
    encode_vector3_3409(values)
}

/// Encode scale data to 0x3409 compressed format.
/// 
/// This is an alias for `encode_vector3_3409` for clarity when encoding scale.
pub fn encode_scale_3409(values: &[Vector3]) -> Result<Vec<u8>, error::Error> {
    encode_vector3_3409(values)
}

// ========================== 0x4409 Quaternion Encoder ======================

/// Encode quaternion data to 0x4409 compressed format.
///
/// This encoder targets EXVS2 Anim v1.2 compatibility. The format matches the
/// validated decode model in `nuanmb_v12/rotate_4409.rs`:
/// - 33-key blocks
/// - vec4<f32> endpoints (block_count + 1)
/// - residual stream encoded using the same residual/kernel family as 0x3409/0x4409 decoders
///
/// Note: This is a lossy encoder.
pub fn encode_rotate_4409(values: &[Vector4]) -> Result<Vec<u8>, error::Error> {
    if values.is_empty() {
        return Err(error::Error::InvalidData);
    }

    let key_count = values.len();
    let blocks = compute_block_count(key_count);
    let endpoint_count = blocks + 1;

    // Step 1: Fit endpoints (use first key of each block, plus final key).
    let mut endpoints = Vec::with_capacity(endpoint_count);
    for block_idx in 0..blocks {
        let start_key = block_idx * 33;
        endpoints.push(values[start_key]);
    }
    endpoints.push(values[key_count - 1]);

    // Step 2: Compute residuals.
    let mut residuals = Vec::with_capacity(key_count);
    let mut max_residual = 0.0f32;
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let block_len = compute_block_len(key_count, block_idx).max(1);
        let t = local as f32 / block_len as f32;

        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let predicted = Vector4 {
            x: e0.x + (e1.x - e0.x) * t,
            y: e0.y + (e1.y - e0.y) * t,
            z: e0.z + (e1.z - e0.z) * t,
            w: e0.w + (e1.w - e0.w) * t,
        };

        let actual = values[key_idx];
        let r = Vector4 {
            x: actual.x - predicted.x,
            y: actual.y - predicted.y,
            z: actual.z - predicted.z,
            w: actual.w - predicted.w,
        };

        max_residual = max_residual
            .max(r.x.abs())
            .max(r.y.abs())
            .max(r.z.abs())
            .max(r.w.abs());
        residuals.push(r);
    }

    // Step 3: Compute base_scale (max absolute residual).
    let base_scale = if max_residual > 1e-6 { max_residual } else { 1.0 };

    // Step 4: Encode residual stream using the kernel residual model.
    let (residual_stream, q_counts) = encode_residuals_vec4(&residuals, key_count, base_scale);

    // Step 5: Pack buffer.
    let mut data = Vec::new();
    data.extend_from_slice(&0x4409u32.to_le_bytes());
    data.extend_from_slice(&(key_count as u32).to_le_bytes());
    data.extend_from_slice(&1.0f32.to_le_bytes()); // unk1
    data.extend_from_slice(&base_scale.to_le_bytes());
    // Observed in-game:
    // - For 2-block curves: flags=2, endpoint_base=0x14.
    // - For 3+ blocks: flags=block_count, endpoint_base=0x18, u32@0x14 = prefix sum of q_counts (excluding last block).
    let flags: u16 = if blocks >= 3 { blocks as u16 } else { 2u16 };
    // bits vary widely in assets, but appear to fall into different ranges depending on the endpoint_base variant.
    // For blocks>=3 (endpoint_base=0x18), shipped assets tend to use values like 0x0038..0x0048 for varyMask=0xF.
    // Use a conservative default that is present in shipped 0x18-variant buffers.
    let vary_mask = {
        let mut mins = [f32::INFINITY; 4];
        let mut maxs = [f32::NEG_INFINITY; 4];
        for q in values {
            mins[0] = mins[0].min(q.x);
            mins[1] = mins[1].min(q.y);
            mins[2] = mins[2].min(q.z);
            mins[3] = mins[3].min(q.w);
            maxs[0] = maxs[0].max(q.x);
            maxs[1] = maxs[1].max(q.y);
            maxs[2] = maxs[2].max(q.z);
            maxs[3] = maxs[3].max(q.w);
        }
        let eps = 1.0e-5f32;
        let mut m = 0u8;
        for i in 0..4 {
            if (maxs[i] - mins[i]).abs() > eps {
                m |= 1u8 << i;
            }
        }
        m
    };
    let bits: u16 = if blocks >= 3 {
        match vary_mask {
            0xF => 0x0041, // common for varying quaternions in shipped 0x18-variant buffers
            0xC => 0x0022, // observed for ZW-only variation in shipped 0x18-variant buffers
            _ => 0x0041,
        }
    } else {
        // Keep the previous default for 2-block curves (common in shipped 0x14-variant buffers).
        0x0035
    };
    data.extend_from_slice(&flags.to_le_bytes()); // flags
    data.extend_from_slice(&bits.to_le_bytes()); // bits
    if blocks >= 3 {
        let prefix_sum_q_counts = q_counts.iter().take(blocks - 1).sum::<usize>() as u32;
        data.extend_from_slice(&prefix_sum_q_counts.to_le_bytes());
    }

    // Endpoints as vec4<f32>.
    for ep in &endpoints {
        data.extend_from_slice(&ep.x.to_le_bytes());
        data.extend_from_slice(&ep.y.to_le_bytes());
        data.extend_from_slice(&ep.z.to_le_bytes());
        data.extend_from_slice(&ep.w.to_le_bytes());
    }

    // Residual stream (already 4-byte aligned).
    data.extend_from_slice(&residual_stream);
    Ok(data)
}

fn encode_residuals_vec4(
    residuals: &[Vector4],
    key_count: usize,
    base_scale: f32,
) -> (Vec<u8>, Vec<usize>) {
    let blocks = compute_block_count(key_count);
    let mut stream = Vec::new();
    let mut q_counts: Vec<usize> = Vec::with_capacity(blocks);

    for block_idx in 0..blocks {
        let block_len = compute_block_len(key_count, block_idx);
        let block_start = stream.len();
        if block_len > 1 {
            let start_key = block_idx * 33 + 1; // Skip first key (endpoint).
            let end_key = (block_idx * 33 + block_len).min(key_count); // Exclusive.

            let mut bx = Vec::new();
            let mut by = Vec::new();
            let mut bz = Vec::new();
            let mut bw = Vec::new();
            for key_idx in start_key..end_key {
                bx.push(residuals[key_idx].x);
                by.push(residuals[key_idx].y);
                bz.push(residuals[key_idx].z);
                bw.push(residuals[key_idx].w);
            }

            encode_residual_component_kernel_fixed(&mut stream, &bx, base_scale);
            encode_residual_component_kernel_fixed(&mut stream, &by, base_scale);
            encode_residual_component_kernel_fixed(&mut stream, &bz, base_scale);
            encode_residual_component_kernel_fixed(&mut stream, &bw, base_scale);
        }
        let block_end = stream.len();
        q_counts.push((block_end - block_start) / 4);
    }

    if stream.is_empty() {
        stream.extend_from_slice(&[0u8; 4]);
        // Single filler word; attribute it to the last block for a consistent prefix sum model.
        if q_counts.is_empty() {
            q_counts.push(1);
        } else {
            let last = q_counts.len() - 1;
            q_counts[last] = q_counts[last].saturating_add(1);
        }
    }
    while (stream.len() % 4) != 0 {
        stream.push(0);
    }
    (stream, q_counts)
}

// ========================== Tests ===========================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim_data::nuanmb_v12::decode_translate_3409;
    use crate::anim_data::nuanmb_v12::decode_rotate_4409;

    #[test]
    fn encode_decode_roundtrip_simple() {
        // Test that encoding and decoding produce similar results
        let original = vec![
            Vector3 { x: 0.0, y: 1.0, z: 0.0 },
            Vector3 { x: 0.1, y: 1.1, z: 0.1 },
            Vector3 { x: 0.2, y: 1.2, z: 0.2 },
            Vector3 { x: 0.3, y: 1.3, z: 0.3 },
        ];

        let encoded = encode_vector3_3409(&original).unwrap();
        
        // Verify header
        assert_eq!(&encoded[0..4], &0x3409u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &4u32.to_le_bytes()); // key_count
        
        let decoded = decode_translate_3409(&encoded).unwrap();

        // Should have same frame count
        assert_eq!(decoded.len(), original.len());

        // Check approximate equality (lossy compression)
        for (i, (orig, dec)) in original.iter().zip(decoded.iter()).enumerate() {
            let err_x = (orig.x - dec.x).abs();
            let err_y = (orig.y - dec.y).abs();
            let err_z = (orig.z - dec.z).abs();
            assert!(
                err_x < 0.1 && err_y < 0.1 && err_z < 0.1,
                "Frame {} error too large: ({}, {}, {})",
                i,
                err_x,
                err_y,
                err_z
            );
        }
    }

    #[test]
    fn encode_decode_roundtrip_multi_block() {
        // Test with more than 33 keys (multiple blocks)
        let mut original = Vec::new();
        for i in 0..100 {
            let t = i as f32 / 99.0;
            original.push(Vector3 {
                x: t * 10.0,
                y: t.sin() * 5.0,
                z: t.cos() * 5.0,
            });
        }

        let encoded = encode_vector3_3409(&original).unwrap();
        let decoded = decode_translate_3409(&encoded).unwrap();

        assert_eq!(decoded.len(), original.len());

        // Check endpoints are preserved exactly
        let first_err = (original[0].x - decoded[0].x).abs()
            + (original[0].y - decoded[0].y).abs()
            + (original[0].z - decoded[0].z).abs();
        assert!(first_err < 0.01, "First frame error: {}", first_err);

        let last_err = (original[99].x - decoded[99].x).abs()
            + (original[99].y - decoded[99].y).abs()
            + (original[99].z - decoded[99].z).abs();
        assert!(last_err < 0.01, "Last frame error: {}", last_err);
    }

    #[test]
    fn encode_single_frame() {
        let original = vec![Vector3 { x: 1.0, y: 2.0, z: 3.0 }];
        let encoded = encode_vector3_3409(&original).unwrap();
        let decoded = decode_translate_3409(&encoded).unwrap();

        assert_eq!(decoded.len(), 1);
        assert!((decoded[0].x - 1.0).abs() < 0.01);
        assert!((decoded[0].y - 2.0).abs() < 0.01);
        assert!((decoded[0].z - 3.0).abs() < 0.01);
    }

    #[test]
    fn encode_empty_fails() {
        let original: Vec<Vector3> = vec![];
        let result = encode_translate_3409(&original);
        assert!(result.is_err());
    }

    #[test]
    fn encode_decode_roundtrip_quaternion_simple() {
        // A simple Y-axis rotation sequence.
        // q = (x,y,z,w) where y = sin(theta/2), w = cos(theta/2).
        let mut original = Vec::new();
        for i in 0..40 {
            let t = i as f32 / 39.0;
            let theta = t * std::f32::consts::PI;
            let (s, c) = (0.5 * theta).sin_cos();
            original.push(Vector4 {
                x: 0.0,
                y: s,
                z: 0.0,
                w: c,
            });
        }

        // The unified entrypoint requires a strict template (65 frames) for now.
        // Use the direct encoder for small synthetic tests.
        let encoded = encode_rotate_4409(&original).unwrap();
        assert_eq!(&encoded[0..4], &0x4409u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &(original.len() as u32).to_le_bytes());

        let decoded = decode_rotate_4409(&encoded).unwrap();
        assert_eq!(decoded.len(), original.len());

        // Loose error bounds: this encoder is lossy and currently prioritizes layout validity.
        for (i, (orig, dec)) in original.iter().zip(decoded.iter()).enumerate() {
            let err = (orig.x - dec.x).abs()
                + (orig.y - dec.y).abs()
                + (orig.z - dec.z).abs()
                + (orig.w - dec.w).abs();
            assert!(err < 0.2, "Frame {} quaternion error too large: {}", i, err);
        }
    }

    #[test]
    fn template_encode_decode_roundtrip_vector3_65() {
        let mut original = Vec::new();
        for i in 0..65 {
            let t = i as f32 / 64.0;
            original.push(Vector3 {
                x: t * 10.0,
                y: (t * std::f32::consts::PI).sin(),
                z: (t * std::f32::consts::PI).cos(),
            });
        }

        let encoded = encode_multiframe_curve(MultiframeCurve::Vector3(&original)).unwrap();
        assert_eq!(&encoded[0..4], &0x3409u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &(original.len() as u32).to_le_bytes());

        let decoded = decode_translate_3409(&encoded).unwrap();
        assert_eq!(decoded.len(), original.len());
    }

    #[test]
    fn template_encode_decode_roundtrip_quaternion_65() {
        let mut original = Vec::new();
        for i in 0..65 {
            let t = i as f32 / 64.0;
            let theta = t * std::f32::consts::PI;
            let (s, c) = (0.5 * theta).sin_cos();
            original.push(Vector4 {
                x: 0.0,
                y: s,
                z: 0.0,
                w: c,
            });
        }

        let encoded = encode_multiframe_curve(MultiframeCurve::Quaternion(&original)).unwrap();
        assert_eq!(&encoded[0..4], &0x4409u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &(original.len() as u32).to_le_bytes());

        let decoded = decode_rotate_4409(&encoded).unwrap();
        assert_eq!(decoded.len(), original.len());
    }

    #[test]
    fn template_encode_decode_roundtrip_vector3_90() {
        let mut original = Vec::new();
        for i in 0..90 {
            let t = i as f32 / 89.0;
            original.push(Vector3 {
                x: t * 3.0,
                y: (t * std::f32::consts::PI).sin() * 2.0,
                z: (t * std::f32::consts::PI).cos() * 2.0,
            });
        }

        let encoded = encode_multiframe_curve(MultiframeCurve::Vector3(&original)).unwrap();
        assert_eq!(&encoded[0..4], &0x3409u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &(original.len() as u32).to_le_bytes());

        let decoded = decode_translate_3409(&encoded).unwrap();
        assert_eq!(decoded.len(), original.len());
    }

    #[test]
    fn template_encode_decode_roundtrip_quaternion_90() {
        let mut original = Vec::new();
        for i in 0..90 {
            let t = i as f32 / 89.0;
            let theta = t * std::f32::consts::PI;
            let (s, c) = (0.5 * theta).sin_cos();
            original.push(Vector4 {
                x: 0.0,
                y: s,
                z: 0.0,
                w: c,
            });
        }

        let encoded = encode_multiframe_curve(MultiframeCurve::Quaternion(&original)).unwrap();
        assert_eq!(&encoded[0..4], &0x4409u32.to_le_bytes());
        assert_eq!(&encoded[4..8], &(original.len() as u32).to_le_bytes());

        let decoded = decode_rotate_4409(&encoded).unwrap();
        assert_eq!(decoded.len(), original.len());
    }
}
