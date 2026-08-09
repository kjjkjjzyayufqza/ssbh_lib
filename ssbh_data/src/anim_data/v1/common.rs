use std::f32::consts::PI;
use std::io::Cursor;
use std::sync::OnceLock;

use binrw::BinRead;
use glam::{Quat, Vec3, quat};

use crate::anim_data::{Vector3, Vector4, error};

pub(super) const G_CURVE_SHORT4_SCALE: f32 = 1.0 / 32767.0;
pub(super) const G_CURVE_INT8_SCALE: f32 = 1.0 / 127.0;
pub(super) const G_CURVE_NIBBLE_SCALE: f32 = 1.0 / 7.0;
pub(super) const G_CURVE_NIBBLE_BIAS: f32 = 8.0;

#[inline]
pub(super) fn read_u16_le(data: &[u8], off: usize) -> Result<u16, error::Error> {
    let mut cursor = Cursor::new(&data[off..]);
    u16::read_le(&mut cursor).map_err(|_| error::Error::InvalidData)
}

#[inline]
pub(super) fn read_u32_le(data: &[u8], off: usize) -> Result<u32, error::Error> {
    let mut cursor = Cursor::new(&data[off..]);
    u32::read_le(&mut cursor).map_err(|_| error::Error::InvalidData)
}

#[inline]
pub(super) fn read_f32_le(data: &[u8], off: usize) -> Result<f32, error::Error> {
    let mut cursor = Cursor::new(&data[off..]);
    f32::read_le(&mut cursor).map_err(|_| error::Error::InvalidData)
}

#[inline]
pub(super) fn align_up(x: usize, align: usize) -> usize {
    if align == 0 {
        return x;
    }
    let m = x % align;
    if m == 0 { x } else { x + (align - m) }
}

pub(super) fn read_vec3_f32_le(data: &[u8], off: usize) -> Result<Vector3, error::Error> {
    Ok(Vector3 {
        x: read_f32_le(data, off)?,
        y: read_f32_le(data, off + 4)?,
        z: read_f32_le(data, off + 8)?,
    })
}

pub(super) fn read_vec4_f32_le(data: &[u8], off: usize) -> Result<Vector4, error::Error> {
    Ok(Vector4 {
        x: read_f32_le(data, off)?,
        y: read_f32_le(data, off + 4)?,
        z: read_f32_le(data, off + 8)?,
        w: read_f32_le(data, off + 12)?,
    })
}

pub(super) struct KernelCache {
    data: Vec<f32>,
    offsets: [usize; 8],
}

impl KernelCache {
    fn generate() -> Self {
        let mut offsets = [0usize; 8];
        let mut accum = 0usize;
        for (i, slot) in offsets.iter_mut().enumerate() {
            *slot = accum;
            let dim = (i + 1) * 4;
            accum += dim * dim;
        }

        let mut data = Vec::with_capacity(3264);
        for v18 in 1..=8 {
            let dim = v18 * 4;
            let scale = (2.0 / dim as f32).sqrt();
            for row in 0..dim {
                for col in 0..dim {
                    let val = ((col as f32 + 0.5) * ((row as f32 + 0.5) * (PI / dim as f32))).cos()
                        * scale;
                    data.push(val);
                }
            }
        }
        KernelCache { data, offsets }
    }

    pub(super) fn row(&self, v18: usize, row_idx: usize) -> &[f32] {
        let dim = v18 * 4;
        let base = self.offsets[v18 - 1];
        // Kernel matrix is dim×dim. Locals beyond dim get a zero basis row
        // (matches Python/lib_3409 and avoids panics on long single residual streams).
        if row_idx >= dim {
            // Static zero rows for dim = 4,8,...,32 (v18 = 1..=8).
            static ZEROS: [f32; 32] = [0.0; 32];
            return &ZEROS[..dim];
        }
        let start = base + row_idx * dim;
        let end = start + dim;
        &self.data[start..end]
    }
}

pub(super) fn kernel() -> &'static KernelCache {
    static CACHE: OnceLock<KernelCache> = OnceLock::new();
    CACHE.get_or_init(KernelCache::generate)
}

pub(super) fn decode_residual_component(
    residual_stream: &[u8],
    stream_offset: usize,
    base_scale: f32,
    local_idx: usize,
    block_len: usize,
) -> Result<(f32, usize), error::Error> {
    if local_idx == 0 || local_idx >= block_len {
        return Ok((0.0, stream_offset));
    }
    let mut curr = stream_offset;
    let word0 = read_u16_le(residual_stream, curr)? as f32;
    let word1 = read_u16_le(residual_stream, curr + 2)? as f32;
    let byte4 = residual_stream
        .get(curr + 4)
        .copied()
        .ok_or(error::Error::InvalidData)? as f32;
    let byte5 = residual_stream
        .get(curr + 5)
        .copied()
        .ok_or(error::Error::InvalidData)?;
    let byte6 = residual_stream
        .get(curr + 6)
        .copied()
        .ok_or(error::Error::InvalidData)?;
    let byte7 = residual_stream
        .get(curr + 7)
        .copied()
        .ok_or(error::Error::InvalidData)?;
    curr += 8;

    let v14 = (byte5 >> 4) as usize;
    let v15 = (byte5 & 0xF) as usize;
    let v12 = (byte6 & 0xF) as usize;
    let v13 = (byte6 >> 4) as usize;
    let v18 = v14 + v15 + v12 + v13;
    if v18 == 0 || v18 > 8 {
        return Err(error::Error::InvalidData);
    }

    let base_amp = word0 * (1.0 / 65536.0) * base_scale;
    let slope = word1 * (1.0 / 65536.0) * base_amp;
    let amp_u8 = byte4;

    let v16 = (byte7 >> 4) as usize;
    let v17 = (byte7 & 0xF) as usize;
    if v16 + v17 > 8 {
        return Err(error::Error::InvalidData);
    }
    let v19 = 8 - v16 - v17;

    let mut w = [0.0f32; 8];
    let mut wi = 0usize;
    for _ in 0..v16 {
        w[wi] = base_amp;
        wi += 1;
    }
    for _ in 0..v17 {
        w[wi] = slope;
        wi += 1;
    }
    let scaled_slope = slope * (amp_u8 / 255.0);
    for _ in 0..v19 {
        w[wi] = scaled_slope;
        wi += 1;
    }

    let basis_row = kernel().row(v18, local_idx - 1);
    let mut acc = 0.0f32;
    let mut entry_idx = 0usize;
    let mut basis_ptr = 0usize;

    for _ in 0..v14 {
        if curr + 8 > residual_stream.len() {
            return Err(error::Error::InvalidData);
        }
        let shorts = [
            i16::from_le_bytes([residual_stream[curr], residual_stream[curr + 1]]) as f32
                * G_CURVE_SHORT4_SCALE,
            i16::from_le_bytes([residual_stream[curr + 2], residual_stream[curr + 3]]) as f32
                * G_CURVE_SHORT4_SCALE,
            i16::from_le_bytes([residual_stream[curr + 4], residual_stream[curr + 5]]) as f32
                * G_CURVE_SHORT4_SCALE,
            i16::from_le_bytes([residual_stream[curr + 6], residual_stream[curr + 7]]) as f32
                * G_CURVE_SHORT4_SCALE,
        ];
        curr += 8;
        let basis = &basis_row[basis_ptr..basis_ptr + 4];
        basis_ptr += 4;
        let weight = w[entry_idx % 8];
        entry_idx += 1;
        for i in 0..4 {
            acc += basis[i] * shorts[i] * weight;
        }
    }

    for _ in 0..v15 {
        if curr + 4 > residual_stream.len() {
            return Err(error::Error::InvalidData);
        }
        let coeff = [
            residual_stream[curr] as i8 as f32 * G_CURVE_INT8_SCALE,
            residual_stream[curr + 1] as i8 as f32 * G_CURVE_INT8_SCALE,
            residual_stream[curr + 2] as i8 as f32 * G_CURVE_INT8_SCALE,
            residual_stream[curr + 3] as i8 as f32 * G_CURVE_INT8_SCALE,
        ];
        curr += 4;
        let basis = &basis_row[basis_ptr..basis_ptr + 4];
        basis_ptr += 4;
        let weight = w[entry_idx % 8];
        entry_idx += 1;
        for i in 0..4 {
            acc += basis[i] * coeff[i] * weight;
        }
    }

    let pair_count = v13 >> 1;
    for _ in 0..pair_count {
        if curr + 4 > residual_stream.len() {
            return Err(error::Error::InvalidData);
        }
        let b = [
            residual_stream[curr],
            residual_stream[curr + 1],
            residual_stream[curr + 2],
            residual_stream[curr + 3],
        ];
        curr += 4;
        let decode_nibble = |val: u8, hi: bool| -> f32 {
            let n = if hi { val >> 4 } else { val & 0xF };
            (n as f32 - G_CURVE_NIBBLE_BIAS) * G_CURVE_NIBBLE_SCALE
        };
        let hi_vec = [
            decode_nibble(b[0], true),
            decode_nibble(b[1], true),
            decode_nibble(b[2], true),
            decode_nibble(b[3], true),
        ];
        let basis_hi = &basis_row[basis_ptr..basis_ptr + 4];
        basis_ptr += 4;
        let weight_hi = w[entry_idx % 8];
        entry_idx += 1;
        for i in 0..4 {
            acc += basis_hi[i] * hi_vec[i] * weight_hi;
        }
        let lo_vec = [
            decode_nibble(b[0], false),
            decode_nibble(b[1], false),
            decode_nibble(b[2], false),
            decode_nibble(b[3], false),
        ];
        let basis_lo = &basis_row[basis_ptr..basis_ptr + 4];
        basis_ptr += 4;
        let weight_lo = w[entry_idx % 8];
        entry_idx += 1;
        for i in 0..4 {
            acc += basis_lo[i] * lo_vec[i] * weight_lo;
        }
    }

    Ok((acc, curr))
}

pub(super) fn decode_residual_vector(
    residual_stream: &[u8],
    start_offset: usize,
    base_scale: f32,
    local_idx: usize,
    comp_bits: usize,
    block_len: usize,
) -> Result<(Vec<f32>, usize), error::Error> {
    let mut result = vec![0.0f32; comp_bits];
    let mut curr = start_offset;
    for c in 0..comp_bits {
        let (val, next) =
            decode_residual_component(residual_stream, curr, base_scale, local_idx, block_len)?;
        result[c] = val;
        curr = next;
    }
    Ok((result, curr))
}

pub(super) fn compute_block_len(key_count: usize, block_idx: usize) -> usize {
    if key_count <= 1 {
        return 1;
    }
    // Must match `compute_block_count` (not the old off-by-one last_block index).
    let last_block = compute_block_count(key_count) - 1;
    if block_idx == last_block {
        key_count - 33 * block_idx - 1
    } else {
        33
    }
}

/// Number of 33-key residual blocks for curve-level `0x3409` / `0x4409`.
///
/// IDA (`sub_1400561E0` / type-9 family) and VS2 samples use:
/// `block_count = ceil((key_count - 1) / 33)`.
///
/// Integer form: `(key_count - 2) / 33 + 1` for `key_count >= 2`.
/// This is critical when `key_count = 33*n + 1` (e.g. 100): the old
/// `(key_count - 1) / 33 + 1` over-counted by one and broke residual layout
/// inference for flags/type=3 buffers.
pub(super) fn compute_block_count(key_count: usize) -> usize {
    if key_count <= 1 {
        1
    } else {
        (key_count - 2) / 33 + 1
    }
}

/// Map a key index into a residual block index.
///
/// When the final block holds more than 33 keys (`key_count = 33*n + 1`),
/// `key_idx / 33` would land past the last block; clamp to `block_count - 1`.
pub(super) fn block_index_for_key(key_idx: usize, block_count: usize) -> usize {
    (key_idx / 33).min(block_count.saturating_sub(1))
}

/// Parsed header of a blocked residual curve (`0x3309` / `0x4309` / `0x3409` / `0x4409`).
///
/// The layout is fully deterministic — no offset inference is required:
///
/// ```text
/// magic          u32
/// key_count      u32
/// frames_per_key f32
/// [0x_309]       frame_indices u8[key_count], align 4
/// base_scale   f32
/// block_count  u16
/// block_words  u16[block_count - 1]  // u32-word offset of block b's residual
///                                    // relative to the residual stream start;
///                                    // 0 marks a block with no residual
///              align 4
/// endpoints    VecN[block_count + 1] // Vec3 for 0x3xxx, Vec4 for 0x4xxx
/// residual     blocks
/// ```
pub(super) struct BlockedHeader {
    pub key_count: usize,
    /// Key → frame mapping for the `0x_309` variants; empty for `0x_409`,
    /// where every key is a frame.
    pub frame_indices: Vec<usize>,
    pub base_scale: f32,
    pub block_count: usize,
    /// Byte offset of each block's residual stream. `None` marks a block that
    /// stores no residual and is sampled as a pure endpoint interpolation.
    pub block_starts: Vec<Option<usize>>,
    pub endpoints_offset: usize,
}

/// Read the header of a blocked residual curve.
///
/// `keyed` selects the `0x_309` variants, which carry a `u8` frame index per key.
/// `components` is 3 for `0x3xxx` (vector) curves and 4 for `0x4xxx` (quaternion).
pub(super) fn read_blocked_header(
    bytes: &[u8],
    magic: u32,
    keyed: bool,
    components: usize,
) -> Result<BlockedHeader, error::Error> {
    if bytes.len() < 12 || read_u32_le(bytes, 0)? != magic {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    if key_count == 0 {
        return Err(error::Error::InvalidData);
    }

    // `nu::DecompressCurve::Decompress` locates a key as `time / frames_per_key`
    // for dense curves, and reads a stored index back as `index * frames_per_key`
    // for the keyed ones. It lets a slow curve sample on a coarser grid without
    // widening the u8 index. Every VS2/EXVS2 buffer observed so far stores 1.0.
    let frames_per_key = read_f32_le(bytes, 8)?;
    if !(frames_per_key.is_finite() && frames_per_key > 0.0) {
        return Err(error::Error::InvalidData);
    }

    let mut frame_indices = Vec::new();
    let mut pos = 12;
    if keyed {
        if pos + key_count > bytes.len() {
            return Err(error::Error::InvalidData);
        }
        frame_indices = bytes[pos..pos + key_count]
            .iter()
            .map(|index| (*index as f32 * frames_per_key).round() as usize)
            .collect();
        pos = align_up(pos + key_count, 4);
    } else if (frames_per_key - 1.0).abs() > 1e-6 {
        // A dense curve stores one key per `frames_per_key` frames. Decoding it
        // as one key per frame would silently retime the clip, and nothing in
        // the corpus exercises the resampling, so refuse rather than guess.
        return Err(error::Error::InvalidData);
    }

    if pos + 6 > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, pos)?;
    let block_count = read_u16_le(bytes, pos + 4)? as usize;
    // The block count is redundant with the key count. A mismatch means the
    // buffer is not the layout this decoder understands, so fail loudly rather
    // than guess an offset and emit plausible-looking garbage.
    if block_count != compute_block_count(key_count) {
        return Err(error::Error::InvalidData);
    }

    let endpoints_offset = align_up(pos + 4 + 2 * block_count, 4);
    let residual_offset = endpoints_offset + components * 4 * (block_count + 1);
    if residual_offset > bytes.len() {
        return Err(error::Error::InvalidData);
    }

    let mut block_starts = Vec::with_capacity(block_count);
    block_starts.push(Some(residual_offset));
    for block_idx in 1..block_count {
        let words = read_u16_le(bytes, pos + 6 + 2 * (block_idx - 1))? as usize;
        if words == 0 {
            block_starts.push(None);
            continue;
        }
        let start = residual_offset + 4 * words;
        if start >= bytes.len() {
            return Err(error::Error::InvalidData);
        }
        block_starts.push(Some(start));
    }

    Ok(BlockedHeader {
        key_count,
        frame_indices,
        base_scale,
        block_count,
        block_starts,
        endpoints_offset,
    })
}

impl BlockedHeader {
    /// Map a key index to its residual block, position within the block, and
    /// the block's interpolation length.
    pub(super) fn key_span(&self, key_idx: usize) -> (usize, usize, usize) {
        let block_idx = block_index_for_key(key_idx, self.block_count);
        let local = key_idx - 33 * block_idx;
        let block_len = compute_block_len(self.key_count, block_idx).max(1);
        (block_idx, local, block_len)
    }

    /// Decode the residual correction for one key. Blocks without a residual
    /// stream contribute zero, leaving a pure endpoint interpolation.
    pub(super) fn key_residual(
        &self,
        bytes: &[u8],
        components: usize,
        block_idx: usize,
        local: usize,
        block_len: usize,
    ) -> Result<Vec<f32>, error::Error> {
        match self.block_starts[block_idx] {
            None => Ok(vec![0.0; components]),
            Some(start) => {
                let (values, _) = decode_residual_vector(
                    bytes,
                    start,
                    self.base_scale,
                    local,
                    components,
                    block_len,
                )?;
                Ok(values)
            }
        }
    }
}

pub(super) fn expand_sparse_vec3(
    frame_indices: &[usize],
    values: &[Vec3],
    total_frames: usize,
) -> Vec<Vec3> {
    if values.is_empty() || frame_indices.is_empty() {
        return vec![Vec3::ZERO];
    }
    let mut frames = vec![values[0]; total_frames];

    let first_f = frame_indices[0];
    for f in 0..first_f.min(total_frames) {
        frames[f] = values[0];
    }

    for i in 0..frame_indices.len().saturating_sub(1) {
        let f0 = frame_indices[i];
        let mut f1 = frame_indices[i + 1];
        if f1 <= f0 {
            continue;
        }
        if f1 >= total_frames {
            f1 = total_frames - 1;
        }
        let span = f1.saturating_sub(f0);
        if span == 0 {
            frames[f0] = values[i];
            continue;
        }
        let [x0, y0, z0] = values[i].to_array();
        let [x1, y1, z1] = values[i + 1].to_array();
        for f in f0..=f1 {
            let t = (f - f0) as f32 / span as f32;
            frames[f] = Vec3 {
                x: x0 + (x1 - x0) * t,
                y: y0 + (y1 - y0) * t,
                z: z0 + (z1 - z0) * t,
            };
        }
    }

    if let Some(&last_f) = frame_indices.last() {
        for f in last_f.min(total_frames)..total_frames {
            let [x, y, z] = values.last().unwrap().to_array();
            frames[f] = Vec3 { x, y, z };
        }
    }
    frames
}

pub(super) fn expand_sparse_quat(
    frame_indices: &[usize],
    values: &[(f32, f32, f32, f32)],
    total_frames: usize,
) -> Vec<Quat> {
    if values.is_empty() || frame_indices.is_empty() {
        return vec![Quat::IDENTITY];
    }
    let mut frames = vec![quat(values[0].0, values[0].1, values[0].2, values[0].3,); total_frames];

    let first_f = frame_indices[0];
    for f in 0..first_f.min(total_frames) {
        frames[f] = quat(values[0].0, values[0].1, values[0].2, values[0].3);
    }

    for i in 0..frame_indices.len().saturating_sub(1) {
        let f0 = frame_indices[i];
        let mut f1 = frame_indices[i + 1];
        if f1 <= f0 {
            continue;
        }
        if f1 >= total_frames {
            f1 = total_frames - 1;
        }
        let span = f1.saturating_sub(f0);
        if span == 0 {
            frames[f0] = quat(values[i].0, values[i].1, values[i].2, values[i].3);
            continue;
        }
        let (x0, y0, z0, w0) = values[i];
        let (x1, y1, z1, w1) = values[i + 1];
        // Interpolate along the shortest arc: q and -q are the same rotation,
        // so an antipodal key pair would otherwise spin the long way around.
        let sign = if x0 * x1 + y0 * y1 + z0 * z1 + w0 * w1 < 0.0 {
            -1.0
        } else {
            1.0
        };
        let (x1, y1, z1, w1) = (sign * x1, sign * y1, sign * z1, sign * w1);
        for f in f0..=f1 {
            let t = (f - f0) as f32 / span as f32;
            frames[f] = quat_normalize(
                x0 + (x1 - x0) * t,
                y0 + (y1 - y0) * t,
                z0 + (z1 - z0) * t,
                w0 + (w1 - w0) * t,
            );
        }
    }

    if let Some(&last_f) = frame_indices.last() {
        for f in last_f.min(total_frames)..total_frames {
            let (x, y, z, w) = *values.last().unwrap();
            frames[f] = quat(x, y, z, w);
        }
    }
    frames
}

pub(super) fn compute_block_count_type9(key_count: usize) -> usize {
    if key_count <= 1 {
        1
    } else {
        (key_count - 2) / 33 + 1
    }
}

pub(super) fn compute_block_len_type9(key_count: usize, block_idx: usize) -> usize {
    if key_count <= 1 {
        1
    } else {
        let last_block = compute_block_count_type9(key_count) - 1;
        if block_idx == last_block {
            key_count - 33 * block_idx - 1
        } else {
            33
        }
    }
}

pub(super) fn quat_normalize(x: f32, y: f32, z: f32, w: f32) -> Quat {
    let n2 = x * x + y * y + z * z + w * w;
    if n2 <= 0.0 {
        return Quat::IDENTITY;
    }
    let inv = 1.0 / n2.sqrt();
    quat(x * inv, y * inv, z * inv, w * inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::f32::consts::PI;

    #[test]
    fn kernel_row_matches_formula() {
        let cache = kernel();
        let first = cache.row(1, 0)[0];
        let expected = ((0.5_f32) * (0.5_f32 * (PI / 4.0))).cos() * (2.0_f32 / 4.0).sqrt();
        assert!(
            (first - expected).abs() < 1e-6,
            "first={}, expected={}",
            first,
            expected
        );

        let row1_col2 = cache.row(1, 1)[2];
        let expected_row1_col2 =
            ((2.5_f32) * (1.5_f32 * (PI / 4.0))).cos() * (2.0_f32 / 4.0).sqrt();
        assert!(
            (row1_col2 - expected_row1_col2).abs() < 1e-6,
            "row1_col2={}, expected={}",
            row1_col2,
            expected_row1_col2
        );
    }

    #[test]
    fn residual_component_advances_cursor_and_decodes() {
        let mut residual = Vec::new();
        residual.extend_from_slice(&0x4000u16.to_le_bytes());
        residual.extend_from_slice(&0x8000u16.to_le_bytes());
        residual.push(255);
        residual.push(0x10);
        residual.push(0x00);
        residual.push(0x00);
        residual.extend_from_slice(&[0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        let base_scale = 1.0;
        let local_idx = 1;
        let block_len = 4;
        let (value, next) =
            decode_residual_component(&residual, 0, base_scale, local_idx, block_len).unwrap();

        assert_eq!(next, 16);
        assert!(value.abs() > 0.0);
        assert!(value.abs() < 1.0);
    }

    /// VS2 residual segmentation: ceil((key_count-1)/33), not (key_count-1)/33+1.
    /// Regression: key_count=100 (33*3+1) must be 3 blocks, not 4.
    #[test]
    fn compute_block_count_matches_ida_ceil_rule() {
        assert_eq!(1, compute_block_count(1));
        assert_eq!(1, compute_block_count(2));
        assert_eq!(1, compute_block_count(34));
        assert_eq!(2, compute_block_count(35));
        assert_eq!(2, compute_block_count(40));
        assert_eq!(2, compute_block_count(67));
        assert_eq!(3, compute_block_count(100));
        assert_eq!(3, compute_block_count_type9(100));
        // Last block for 100 keys spans keys 66..99 → block_len 33.
        assert_eq!(33, compute_block_len(100, 0));
        assert_eq!(33, compute_block_len(100, 1));
        assert_eq!(33, compute_block_len(100, 2));
    }

    /// The endpoint table starts on a 4-byte boundary after the block word list,
    /// so `block_count` alone determines where the endpoints begin.
    #[test]
    fn blocked_header_endpoint_offset_follows_block_count() {
        // key_count, block_count, endpoints offset for a dense 0x_409 curve.
        for (key_count, block_count, expected) in [
            (35usize, 2usize, 20usize),
            (100, 3, 24),
            (133, 4, 24),
            (158, 5, 28),
        ] {
            assert_eq!(block_count, compute_block_count(key_count));
            assert_eq!(expected, align_up(12 + 4 + 2 * block_count, 4));
        }
    }

    /// A zero word offset marks a block with no residual stream, which must decode
    /// as a pure endpoint interpolation rather than aliasing onto block 0.
    #[test]
    fn blocked_header_zero_word_marks_empty_block() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x4409u32.to_le_bytes());
        bytes.extend_from_slice(&100u32.to_le_bytes());
        bytes.extend_from_slice(&1.0f32.to_le_bytes());
        bytes.extend_from_slice(&0.5f32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes());
        bytes.extend_from_slice(&7u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.resize(24 + 4 * 16 + 64, 0);

        let header = read_blocked_header(&bytes, 0x4409, false, 4).unwrap();
        assert_eq!(3, header.block_count);
        assert_eq!(24, header.endpoints_offset);
        assert_eq!(vec![Some(88), Some(88 + 4 * 7), None], header.block_starts);
        assert_eq!(
            vec![0.0; 4],
            header.key_residual(&bytes, 4, 2, 1, 33).unwrap()
        );
    }
}
