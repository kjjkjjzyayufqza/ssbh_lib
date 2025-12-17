// -----------------------------------------------------------------------------
// Anim v1.2 (nuanmb) decoding helpers aligned with Python reference tools.
// These utilities mirror test/lib_3409.py and friends to ensure parity with
// the Python decompressor while avoiding hardcoded kernel tables.
// -----------------------------------------------------------------------------

use std::f32::consts::PI;
use std::io::Cursor;
use std::sync::OnceLock;

use binrw::BinRead;

use crate::anim_data::{bitutils::BitReader, error, Vector3, Vector4};

const G_CURVE_SHORT4_SCALE: f32 = 3.051_850_9e-5;
const G_CURVE_INT8_SCALE: f32 = 0.007_874_015_7;
const G_CURVE_NIBBLE_SCALE: f32 = 0.142_857_15;
const G_CURVE_NIBBLE_BIAS: f32 = 8.0;

#[inline]
fn read_u16_le(data: &[u8], off: usize) -> Result<u16, error::Error> {
    let mut cursor = Cursor::new(&data[off..]);
    u16::read_le(&mut cursor).map_err(|_| error::Error::InvalidData)
}

#[inline]
fn read_u32_le(data: &[u8], off: usize) -> Result<u32, error::Error> {
    let mut cursor = Cursor::new(&data[off..]);
    u32::read_le(&mut cursor).map_err(|_| error::Error::InvalidData)
}

#[inline]
fn read_f32_le(data: &[u8], off: usize) -> Result<f32, error::Error> {
    let mut cursor = Cursor::new(&data[off..]);
    f32::read_le(&mut cursor).map_err(|_| error::Error::InvalidData)
}

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

fn read_vec3_f32_le(data: &[u8], off: usize) -> Result<Vector3, error::Error> {
    Ok(Vector3 {
        x: read_f32_le(data, off)?,
        y: read_f32_le(data, off + 4)?,
        z: read_f32_le(data, off + 8)?,
    })
}

fn read_vec4_f32_le(data: &[u8], off: usize) -> Result<Vector4, error::Error> {
    Ok(Vector4 {
        x: read_f32_le(data, off)?,
        y: read_f32_le(data, off + 4)?,
        z: read_f32_le(data, off + 8)?,
        w: read_f32_le(data, off + 12)?,
    })
}

// --------------------------- Kernel Cache -------------------------------

struct KernelCache {
    // Flattened matrices for v18 = 1..8, lengths: (4^2 + 8^2 + ... + 32^2) = 3264.
    data: Vec<f32>,
    offsets: [usize; 8],
}

impl KernelCache {
    fn generate() -> Self {
        // Offsets per v18: sum((4*i)^2) prefix.
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
                    let val = ((col as f32 + 0.5) * ((row as f32 + 0.5) * (PI / dim as f32))).cos() * scale;
                    data.push(val);
                }
            }
        }
        KernelCache { data, offsets }
    }

    fn row(&self, v18: usize, row_idx: usize) -> &[f32] {
        let dim = v18 * 4;
        let base = self.offsets[v18 - 1];
        let start = base + row_idx * dim;
        let end = start + dim;
        &self.data[start..end]
    }
}

fn kernel() -> &'static KernelCache {
    static CACHE: OnceLock<KernelCache> = OnceLock::new();
    CACHE.get_or_init(KernelCache::generate)
}

// -------------------------- Residual decode -----------------------------

fn decode_residual_component(
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
    let byte4 = residual_stream.get(curr + 4).copied().ok_or(error::Error::InvalidData)? as f32;
    let byte5 = residual_stream.get(curr + 5).copied().ok_or(error::Error::InvalidData)?;
    let byte6 = residual_stream.get(curr + 6).copied().ok_or(error::Error::InvalidData)?;
    let byte7 = residual_stream.get(curr + 7).copied().ok_or(error::Error::InvalidData)?;
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

    // 16-bit loop
    for _ in 0..v14 {
        if curr + 8 > residual_stream.len() {
            return Err(error::Error::InvalidData);
        }
        let shorts = [
            i16::from_le_bytes([residual_stream[curr], residual_stream[curr + 1]]) as f32 * G_CURVE_SHORT4_SCALE,
            i16::from_le_bytes([residual_stream[curr + 2], residual_stream[curr + 3]]) as f32 * G_CURVE_SHORT4_SCALE,
            i16::from_le_bytes([residual_stream[curr + 4], residual_stream[curr + 5]]) as f32 * G_CURVE_SHORT4_SCALE,
            i16::from_le_bytes([residual_stream[curr + 6], residual_stream[curr + 7]]) as f32 * G_CURVE_SHORT4_SCALE,
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

    // 8-bit loop
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

    // 4-bit pairs
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
        // hi
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
        // lo
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

fn decode_residual_vector(
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
        let (val, next) = decode_residual_component(residual_stream, curr, base_scale, local_idx, block_len)?;
        result[c] = val;
        curr = next;
    }
    Ok((result, curr))
}

// --------------------------- Block helpers ------------------------------

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

fn compute_block_count(key_count: usize) -> usize {
    if key_count <= 1 {
        1
    } else {
        (key_count - 1) / 33 + 1
    }
}

fn compute_block_qcounts(
    payload: &[u8],
    residual_start0: usize,
    base_scale: f32,
    comp_bits: usize,
    key_count: usize,
) -> Result<Vec<usize>, error::Error> {
    if key_count <= 1 {
        return Ok(vec![0]);
    }
    let blocks = compute_block_count(key_count);
    let mut q_counts = Vec::with_capacity(blocks);
    let mut cursor = residual_start0;
    for block_idx in 0..blocks {
        let block_len = compute_block_len(key_count, block_idx);
        if block_len <= 1 {
            q_counts.push(0);
            continue;
        }
        let (_, end_off) = decode_residual_vector(payload, cursor, base_scale, 1, comp_bits, block_len)?;
        let delta = end_off - cursor;
        if delta == 0 || delta % 4 != 0 {
            return Err(error::Error::InvalidData);
        }
        q_counts.push(delta / 4);
        cursor = end_off;
    }
    Ok(q_counts)
}

fn expand_sparse_vec3(
    frame_indices: &[usize],
    values: &[(f32, f32, f32)],
    total_frames: usize,
) -> Vec<Vector3> {
    if values.is_empty() || frame_indices.is_empty() {
        return vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }];
    }
    let mut frames = vec![Vector3 {
        x: values[0].0,
        y: values[0].1,
        z: values[0].2,
    }; total_frames];

    let first_f = frame_indices[0];
    for f in 0..first_f.min(total_frames) {
        frames[f] = Vector3 {
            x: values[0].0,
            y: values[0].1,
            z: values[0].2,
        };
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
            frames[f0] = Vector3 {
                x: values[i].0,
                y: values[i].1,
                z: values[i].2,
            };
            continue;
        }
        let (x0, y0, z0) = values[i];
        let (x1, y1, z1) = values[i + 1];
        for f in f0..=f1 {
            let t = (f - f0) as f32 / span as f32;
            frames[f] = Vector3 {
                x: x0 + (x1 - x0) * t,
                y: y0 + (y1 - y0) * t,
                z: z0 + (z1 - z0) * t,
            };
        }
    }

    if let Some(&last_f) = frame_indices.last() {
        for f in last_f.min(total_frames)..total_frames {
            let (x, y, z) = *values.last().unwrap();
            frames[f] = Vector3 { x, y, z };
        }
    }
    frames
}

fn expand_sparse_quat(
    frame_indices: &[usize],
    values: &[(f32, f32, f32, f32)],
    total_frames: usize,
) -> Vec<Vector4> {
    if values.is_empty() || frame_indices.is_empty() {
        return vec![Vector4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }];
    }
    let mut frames = vec![Vector4 {
        x: values[0].0,
        y: values[0].1,
        z: values[0].2,
        w: values[0].3,
    }; total_frames];

    let first_f = frame_indices[0];
    for f in 0..first_f.min(total_frames) {
        frames[f] = Vector4 {
            x: values[0].0,
            y: values[0].1,
            z: values[0].2,
            w: values[0].3,
        };
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
            frames[f0] = Vector4 {
                x: values[i].0,
                y: values[i].1,
                z: values[i].2,
                w: values[i].3,
            };
            continue;
        }
        let (x0, y0, z0, w0) = values[i];
        let (x1, y1, z1, w1) = values[i + 1];
        for f in f0..=f1 {
            let t = (f - f0) as f32 / span as f32;
            frames[f] = Vector4 {
                x: x0 + (x1 - x0) * t,
                y: y0 + (y1 - y0) * t,
                z: z0 + (z1 - z0) * t,
                w: w0 + (w1 - w0) * t,
            };
        }
    }

    if let Some(&last_f) = frame_indices.last() {
        for f in last_f.min(total_frames)..total_frames {
            let (x, y, z, w) = *values.last().unwrap();
            frames[f] = Vector4 { x, y, z, w };
        }
    }
    frames
}

// ----------------------------- Decoders ---------------------------------

pub fn decode_translate_3200(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3200 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * unk1).round() as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    let mut values = Vec::with_capacity(key_count);
    for i in 0..key_count {
        values.push(read_vec3_f32_le(bytes, pos + i * 12)?);
    }
    let last_frame = *frame_indices.iter().max().unwrap_or(&0);
    let total_frames = last_frame + 1;
    Ok(expand_sparse_vec3(
        &frame_indices,
        &values
            .iter()
            .map(|v| (v.x, v.y, v.z))
            .collect::<Vec<_>>(),
        total_frames,
    ))
}

pub fn decode_translate_3208(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3208 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }]);
    }
    if key_count > 34 {
        return Err(error::Error::InvalidData);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * unk1).round() as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    let base_scale = read_f32_le(bytes, pos)?;
    let endpoint0 = read_vec3_f32_le(bytes, pos + 4)?;
    let endpoint1 = read_vec3_f32_le(bytes, pos + 16)?;
    let residual_off = pos + 28;
    if residual_off > bytes.len() || residual_off % 4 != 0 {
        return Err(error::Error::InvalidData);
    }
    let block_len = key_count.saturating_sub(1).max(1);
    let mut key_values = Vec::with_capacity(key_count);
    for local in 0..key_count {
        let t = local as f32 / block_len as f32;
        let kx = endpoint0.x + (endpoint1.x - endpoint0.x) * t;
        let ky = endpoint0.y + (endpoint1.y - endpoint0.y) * t;
        let kz = endpoint0.z + (endpoint1.z - endpoint0.z) * t;
        let (r_vec, _) = decode_residual_vector(bytes, residual_off, base_scale, local, 3, block_len)?;
        key_values.push((kx + r_vec[0], ky + r_vec[1], kz + r_vec[2]));
    }
    let frames = expand_sparse_vec3(&frame_indices, &key_values, frame_indices.iter().copied().max().unwrap_or(0) + 1);
    Ok(frames)
}

pub fn decode_translate_3300(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3300 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((bytes.get(pos).copied().ok_or(error::Error::InvalidData)? as f32 * unk1).round() as usize);
        pos += 1;
    }
    pos = align_up(pos, 4);
    let mut values = Vec::with_capacity(key_count);
    for i in 0..key_count {
        values.push(read_vec3_f32_le(bytes, pos + i * 12)?);
    }
    let frames = expand_sparse_vec3(
        &frame_indices,
        &values
            .iter()
            .map(|v| (v.x, v.y, v.z))
            .collect::<Vec<_>>(),
        frame_indices.iter().copied().max().unwrap_or(0) + 1,
    );
    Ok(frames)
}

pub fn decode_translate_3308(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3308 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }]);
    }
    if key_count > 33 {
        return Err(error::Error::InvalidData);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((bytes.get(pos).copied().ok_or(error::Error::InvalidData)? as f32 * unk1).round() as usize);
        pos += 1;
    }
    pos = align_up(pos, 4);
    let base_scale = read_f32_le(bytes, pos)?;
    let endpoint0 = read_vec3_f32_le(bytes, pos + 4)?;
    let endpoint1 = read_vec3_f32_le(bytes, pos + 16)?;
    let residual_off = pos + 28;
    if residual_off > bytes.len() || residual_off % 4 != 0 {
        return Err(error::Error::InvalidData);
    }
    let block_len = key_count.saturating_sub(1).max(1);
    let mut key_values = Vec::with_capacity(key_count);
    for local in 0..key_count {
        let t = local as f32 / block_len as f32;
        let kx = endpoint0.x + (endpoint1.x - endpoint0.x) * t;
        let ky = endpoint0.y + (endpoint1.y - endpoint0.y) * t;
        let kz = endpoint0.z + (endpoint1.z - endpoint0.z) * t;
        let (r_vec, _) = decode_residual_vector(bytes, residual_off, base_scale, local, 3, block_len)?;
        key_values.push((kx + r_vec[0], ky + r_vec[1], kz + r_vec[2]));
    }
    let frames = expand_sparse_vec3(&frame_indices, &key_values, frame_indices.iter().copied().max().unwrap_or(0) + 1);
    Ok(frames)
}

fn compute_block_count_type9(key_count: usize) -> usize {
    if key_count <= 1 {
        1
    } else {
        (key_count - 2) / 33 + 1
    }
}

fn compute_block_len_type9(key_count: usize, block_idx: usize) -> usize {
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

pub fn decode_translate_3209(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3209 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * unk1).round() as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    if pos + 8 > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, pos)?;
    let block_count = read_u16_le(bytes, pos + 4)? as usize;
    let expected_blocks = compute_block_count_type9(key_count);
    if block_count != expected_blocks || block_count == 0 {
        return Err(error::Error::InvalidData);
    }
    let mut block_words = Vec::with_capacity(block_count.saturating_sub(1));
    let mut bw_off = pos + 6;
    for _ in 0..block_count.saturating_sub(1) {
        block_words.push(read_u16_le(bytes, bw_off)? as usize);
        bw_off += 2;
    }
    let endpoint_base = align_up(pos + 4 + 2 * block_count, 4);
    let endpoint_count = block_count + 1;
    let endpoints_size = endpoint_count * 12;
    if endpoint_base + endpoints_size > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let mut endpoints = Vec::with_capacity(endpoint_count);
    for i in 0..endpoint_count {
        endpoints.push(read_vec3_f32_le(bytes, endpoint_base + i * 12)?);
    }
    let residual_off = endpoint_base + endpoints_size;
    if residual_off > bytes.len() || residual_off % 4 != 0 {
        return Err(error::Error::InvalidData);
    }
    let mut residual_starts = Vec::with_capacity(block_count);
    residual_starts.push(residual_off);
    for w in &block_words {
        residual_starts.push(residual_off + 4 * *w);
    }

    let mut key_values = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let mut block_len = compute_block_len_type9(key_count, block_idx);
        if block_len == 0 {
            block_len = 1;
        }
        let t_block = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let kx = e0.x + (e1.x - e0.x) * t_block;
        let ky = e0.y + (e1.y - e0.y) * t_block;
        let kz = e0.z + (e1.z - e0.z) * t_block;
        let rs = residual_starts[block_idx];
        let (r_vec, _) = decode_residual_vector(bytes, rs, base_scale, local, 3, block_len)?;
        key_values.push((kx + r_vec[0], ky + r_vec[1], kz + r_vec[2]));
    }
    let frames = expand_sparse_vec3(&frame_indices, &key_values, frame_indices.iter().copied().max().unwrap_or(0) + 1);
    Ok(frames)
}

pub fn decode_translate_3309(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3309 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((bytes.get(pos).copied().ok_or(error::Error::InvalidData)? as f32 * unk1).round() as usize);
        pos += 1;
    }
    pos = align_up(pos, 4);
    if pos + 8 > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, pos)?;
    let block_count = read_u16_le(bytes, pos + 4)? as usize;
    let expected_blocks = compute_block_count_type9(key_count);
    if block_count != expected_blocks || block_count == 0 {
        return Err(error::Error::InvalidData);
    }
    let mut block_words = Vec::with_capacity(block_count.saturating_sub(1));
    let mut bw_off = pos + 6;
    for _ in 0..block_count.saturating_sub(1) {
        block_words.push(read_u16_le(bytes, bw_off)? as usize);
        bw_off += 2;
    }
    let endpoint_base = align_up(pos + 2 * block_count + 4, 4);
    let endpoint_count = block_count + 1;
    let endpoints_size = endpoint_count * 12;
    if endpoint_base + endpoints_size > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let mut endpoints = Vec::with_capacity(endpoint_count);
    for i in 0..endpoint_count {
        endpoints.push(read_vec3_f32_le(bytes, endpoint_base + i * 12)?);
    }
    let residual_off = endpoint_base + endpoints_size;
    if residual_off > bytes.len() || residual_off % 4 != 0 {
        return Err(error::Error::InvalidData);
    }
    let mut residual_starts = Vec::with_capacity(block_count);
    residual_starts.push(residual_off);
    for w in &block_words {
        residual_starts.push(residual_off + 4 * *w);
    }

    let mut key_values = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let mut block_len = compute_block_len_type9(key_count, block_idx);
        if block_len == 0 {
            block_len = 1;
        }
        let t_block = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let kx = e0.x + (e1.x - e0.x) * t_block;
        let ky = e0.y + (e1.y - e0.y) * t_block;
        let kz = e0.z + (e1.z - e0.z) * t_block;
        let rs = residual_starts[block_idx];
        let (r_vec, _) = decode_residual_vector(bytes, rs, base_scale, local, 3, block_len)?;
        key_values.push((kx + r_vec[0], ky + r_vec[1], kz + r_vec[2]));
    }
    let frames = expand_sparse_vec3(&frame_indices, &key_values, frame_indices.iter().copied().max().unwrap_or(0) + 1);
    Ok(frames)
}

pub fn decode_translate_3400(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3400 {
        return Err(error::Error::InvalidData);
    }
    let frame_count = read_u32_le(bytes, 4)? as usize;
    let mut pos = 12;
    let mut frames = Vec::with_capacity(frame_count);
    for i in 0..frame_count {
        frames.push(read_vec3_f32_le(bytes, pos + i * 12)?);
    }
    Ok(frames)
}

pub fn decode_translate_3408(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3408 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let _unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }]);
    }
    let blocks = compute_block_count(key_count);
    let endpoint_count = blocks + 1;
    let endpoints_size = endpoint_count * 12;
    let scan_start = 12;
    let scan_end = (scan_start + endpoints_size + 0x40).min(bytes.len());

    let mut best: Option<(Vec<Vector3>, f32, usize, usize, Vec<usize>)> = None;
    let mut best_key: Option<(usize, isize, usize)> = None;

    for endpoint_base in (scan_start..=scan_end).step_by(4) {
        if endpoint_base + endpoints_size > bytes.len() {
            continue;
        }
        let mut endpoints = Vec::with_capacity(endpoint_count);
        for i in 0..endpoint_count {
            endpoints.push(read_vec3_f32_le(bytes, endpoint_base + i * 12)?);
        }
        let end_ep = endpoint_base + endpoints_size;
        for base_scale_off in (end_ep..=(end_ep + 0x10).min(bytes.len().saturating_sub(4))).step_by(4) {
            let base_scale = read_f32_le(bytes, base_scale_off)?;
            for residual_off in (base_scale_off + 4..=(base_scale_off + 0x80).min(bytes.len())).step_by(4) {
                if residual_off >= bytes.len() {
                    continue;
                }
                for comp_bits in [3usize, 2, 1, 4] {
                    let q_counts = match compute_block_qcounts(bytes, residual_off, base_scale, comp_bits, key_count) {
                        Ok(q) => q,
                        Err(_) => continue,
                    };
                    let end_off = residual_off + 4 * q_counts.iter().sum::<usize>();
                    if end_off > bytes.len() {
                        continue;
                    }
                    let slack = bytes.len() - end_off;
                    let cand_key = (slack, -(comp_bits as isize), residual_off);
                    if best.is_none() || cand_key < best_key.unwrap() {
                        best = Some((endpoints.clone(), base_scale, comp_bits, residual_off, q_counts));
                        best_key = Some(cand_key);
                    }
                    break;
                }
            }
        }
    }

    let (endpoints, base_scale, comp_bits, residual_off, q_counts) = best.ok_or(error::Error::InvalidData)?;
    let mut prefix_words = Vec::with_capacity(q_counts.len() + 1);
    prefix_words.push(0usize);
    for q in &q_counts {
        prefix_words.push(prefix_words.last().copied().unwrap_or(0) + *q);
    }

    let mut out = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let mut block_len = compute_block_len(key_count, block_idx);
        if block_len == 0 {
            block_len = 1;
        }
        let t_block = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let kx = e0.x + (e1.x - e0.x) * t_block;
        let ky = e0.y + (e1.y - e0.y) * t_block;
        let kz = e0.z + (e1.z - e0.z) * t_block;
        let rs = residual_off + 4 * prefix_words[block_idx];
        let (r_vec, _) = decode_residual_vector(bytes, rs, base_scale, local, comp_bits, block_len)?;
        out.push(Vector3 {
            x: kx + r_vec.get(0).copied().unwrap_or(0.0),
            y: ky + r_vec.get(1).copied().unwrap_or(0.0),
            z: kz + r_vec.get(2).copied().unwrap_or(0.0),
        });
    }
    Ok(out)
}

pub fn decode_rotate_4300(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4300 {
        return Err(error::Error::InvalidData);
    }
    let frame_count = read_u32_le(bytes, 4)? as usize;
    // Some files include two f32 values after frame_count (unk1, unk2),
    // while others appear to include only unk1.
    // Determine the quaternion stream start based on total buffer size.
    let pos = if bytes.len() >= 16 + frame_count * 16 && bytes.len() != 12 + frame_count * 16 {
        16
    } else {
        12
    };
    let mut frames = Vec::with_capacity(frame_count);
    let mut pos = pos;
    for _ in 0..frame_count {
        let mut q = read_vec4_f32_le(bytes, pos)?;
        pos += 16;
        let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
        if len2 > 0.0 {
            let inv = 1.0 / len2.sqrt();
            q.x *= inv;
            q.y *= inv;
            q.z *= inv;
            q.w *= inv;
        } else {
            q = Vector4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            };
        }
        frames.push(q);
    }
    Ok(frames)
}

pub fn decode_rotate_4400(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4400 {
        return Err(error::Error::InvalidData);
    }
    let frame_count = read_u32_le(bytes, 4)? as usize;
    let mut pos = 12;
    let mut frames = Vec::with_capacity(frame_count);
    for i in 0..frame_count {
        let mut q = if pos + 16 <= bytes.len() {
            read_vec4_f32_le(bytes, pos)?
        } else if pos + 12 <= bytes.len() && i + 1 == frame_count {
            // handle variant with missing w
            let v = read_vec3_f32_le(bytes, pos)?;
            Vector4 {
                x: v.x,
                y: v.y,
                z: v.z,
                w: 1.0,
            }
        } else {
            return Err(error::Error::InvalidData);
        };
        pos += 16.min(bytes.len().saturating_sub(pos));
        let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
        if len2 > 0.0 {
            let inv = 1.0 / len2.sqrt();
            q.x *= inv;
            q.y *= inv;
            q.z *= inv;
            q.w *= inv;
        } else {
            q = Vector4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            };
        }
        frames.push(q);
    }
    Ok(frames)
}

pub fn decode_rotate_4200(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4200 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    if key_count == 0 {
        return Ok(vec![Vector4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push(read_u16_le(bytes, pos)? as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    let mut key_vals = Vec::with_capacity(key_count);
    for i in 0..key_count {
        key_vals.push(read_vec4_f32_le(bytes, pos + i * 16)?);
    }
    let total_frames = frame_indices.iter().copied().max().unwrap_or(0) + 1;
    Ok(expand_sparse_quat(
        &frame_indices,
        &key_vals.iter().map(|q| (q.x, q.y, q.z, q.w)).collect::<Vec<_>>(),
        total_frames,
    ))
}

pub fn decode_rotate_4208(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4208 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }]);
    }
    if key_count > 33 {
        return Err(error::Error::InvalidData);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * unk1).round() as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    let base_scale = read_f32_le(bytes, pos)?;
    let quat0 = read_vec4_f32_le(bytes, pos + 4)?;
    let quat1 = read_vec4_f32_le(bytes, pos + 20)?;
    let residual_off = pos + 36;
    if residual_off > bytes.len() || residual_off % 4 != 0 {
        return Err(error::Error::InvalidData);
    }
    let block_len = key_count.saturating_sub(1).max(1);
    let mut key_quats = Vec::with_capacity(key_count);
    for local in 0..key_count {
        let t = local as f32 / block_len as f32;
        let k = Vector4 {
            x: quat0.x + (quat1.x - quat0.x) * t,
            y: quat0.y + (quat1.y - quat0.y) * t,
            z: quat0.z + (quat1.z - quat0.z) * t,
            w: quat0.w + (quat1.w - quat0.w) * t,
        };
        let (r_vec, _) = decode_residual_vector(bytes, residual_off, base_scale, local, 4, block_len)?;
        let mut q = Vector4 {
            x: k.x + r_vec[0],
            y: k.y + r_vec[1],
            z: k.z + r_vec[2],
            w: k.w + r_vec[3],
        };
        let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
        if len2 > 0.0 {
            let inv = 1.0 / len2.sqrt();
            q.x *= inv;
            q.y *= inv;
            q.z *= inv;
            q.w *= inv;
        }
        key_quats.push((q.x, q.y, q.z, q.w));
    }
    let total_frames = frame_indices.iter().copied().max().unwrap_or(0) + 1;
    Ok(expand_sparse_quat(&frame_indices, &key_quats, total_frames))
}

pub fn decode_rotate_4209(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4209 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * unk1).round() as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    if pos + 8 > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, pos)?;
    let block_count = read_u16_le(bytes, pos + 4)? as usize;
    let expected_blocks = compute_block_count_type9(key_count);
    if block_count != expected_blocks || block_count == 0 {
        return Err(error::Error::InvalidData);
    }
    let mut block_words = Vec::with_capacity(block_count.saturating_sub(1));
    let mut bw_off = pos + 6;
    for _ in 0..block_count.saturating_sub(1) {
        block_words.push(read_u16_le(bytes, bw_off)? as usize);
        bw_off += 2;
    }
    let endpoint_base = align_up(pos + 4 + 2 * block_count, 4);
    let endpoint_count = block_count + 1;
    let endpoints_size = endpoint_count * 16;
    if endpoint_base + endpoints_size > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let mut endpoints = Vec::with_capacity(endpoint_count);
    for i in 0..endpoint_count {
        endpoints.push(read_vec4_f32_le(bytes, endpoint_base + i * 16)?);
    }
    let residual_off = endpoint_base + endpoints_size;
    if residual_off > bytes.len() || residual_off % 4 != 0 {
        return Err(error::Error::InvalidData);
    }
    let mut residual_starts = Vec::with_capacity(block_count);
    residual_starts.push(residual_off);
    for w in &block_words {
        residual_starts.push(residual_off + 4 * *w);
    }
    let mut key_quats = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let mut block_len = compute_block_len_type9(key_count, block_idx);
        if block_len == 0 {
            block_len = 1;
        }
        let t = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let k = Vector4 {
            x: e0.x + (e1.x - e0.x) * t,
            y: e0.y + (e1.y - e0.y) * t,
            z: e0.z + (e1.z - e0.z) * t,
            w: e0.w + (e1.w - e0.w) * t,
        };
        let rs = residual_starts[block_idx];
        let (r_vec, _) = decode_residual_vector(bytes, rs, base_scale, local, 4, block_len)?;
        let mut q = Vector4 {
            x: k.x + r_vec[0],
            y: k.y + r_vec[1],
            z: k.z + r_vec[2],
            w: k.w + r_vec[3],
        };
        let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
        if len2 > 0.0 {
            let inv = 1.0 / len2.sqrt();
            q.x *= inv;
            q.y *= inv;
            q.z *= inv;
            q.w *= inv;
        }
        key_quats.push((q.x, q.y, q.z, q.w));
    }
    let total_frames = frame_indices.iter().copied().max().unwrap_or(0) + 1;
    Ok(expand_sparse_quat(&frame_indices, &key_quats, total_frames))
}

pub fn decode_rotate_4308(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4308 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let _unk1 = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vector4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }]);
    }
    // Read frame indices u8
    let mut pos = 12;
    if pos + key_count > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let frame_indices: Vec<usize> = bytes[pos..pos + key_count].iter().map(|v| *v as usize).collect();
    pos = align_up(pos + key_count, 4);
    if pos + 36 > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, pos)?;
    let quat0 = read_vec4_f32_le(bytes, pos + 4)?;
    let quat1 = read_vec4_f32_le(bytes, pos + 20)?;
    pos += 36;
    let payload = &bytes[pos..];
    // Variant A: nibble blend (rare). Detect by exact length.
    let nibble_size = (key_count * 4 + 1) / 2;
    if payload.len() == nibble_size {
        // t_raw in 0..15, t = 1 - raw/15
        let mut key_quats = Vec::with_capacity(key_count);
        for i in 0..key_count {
            let byte = payload[i / 2];
            let raw = if i % 2 == 0 { byte & 0xF } else { byte >> 4 };
            let t = 1.0 - (raw as f32 / 15.0);
            let qx = quat0.x + (quat1.x - quat0.x) * t;
            let qy = quat0.y + (quat1.y - quat0.y) * t;
            let qz = quat0.z + (quat1.z - quat0.z) * t;
            let qw = quat0.w + (quat1.w - quat0.w) * t;
            let len2 = qx * qx + qy * qy + qz * qz + qw * qw;
            let inv = if len2 > 0.0 { 1.0 / len2.sqrt() } else { 1.0 };
            key_quats.push((qx * inv, qy * inv, qz * inv, qw * inv));
        }
        let total_frames = frame_indices.iter().copied().max().unwrap_or(0) + 1;
        return Ok(expand_sparse_quat(&frame_indices, &key_quats, total_frames));
    }

    // Variant B: residual/kernel single block (verified).
    let block_len = key_count.saturating_sub(1).max(1);
    let mut key_quats = Vec::with_capacity(key_count);
    for local in 0..key_count {
        let t = local as f32 / block_len as f32;
        let k = Vector4 {
            x: quat0.x + (quat1.x - quat0.x) * t,
            y: quat0.y + (quat1.y - quat0.y) * t,
            z: quat0.z + (quat1.z - quat0.z) * t,
            w: quat0.w + (quat1.w - quat0.w) * t,
        };
        let (r_vec, _) = decode_residual_vector(bytes, pos, base_scale, local, 4, block_len)?;
        let mut q = Vector4 {
            x: k.x + r_vec[0],
            y: k.y + r_vec[1],
            z: k.z + r_vec[2],
            w: k.w + r_vec[3],
        };
        let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
        if len2 > 0.0 {
            let inv = 1.0 / len2.sqrt();
            q.x *= inv;
            q.y *= inv;
            q.z *= inv;
            q.w *= inv;
        }
        key_quats.push((q.x, q.y, q.z, q.w));
    }
    let total_frames = frame_indices.iter().copied().max().unwrap_or(0) + 1;
    Ok(expand_sparse_quat(&frame_indices, &key_quats, total_frames))
}

pub fn decode_rotate_4408(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4408 {
        return Err(error::Error::InvalidData);
    }
    let frame_count = read_u32_le(bytes, 4)? as usize;
    let _unk1 = read_f32_le(bytes, 8)?;
    let _unk2 = read_f32_le(bytes, 12)?;
    if frame_count == 0 {
        return Ok(vec![Vector4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }]);
    }
    let mut pos = 16;
    if pos + 32 > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let defaults = [read_vec4_f32_le(bytes, pos)?, read_vec4_f32_le(bytes, pos + 16)?];
    pos += 32;
    let payload = &bytes[pos..];

    // Variant A: fixed 10 bpc bitstream (5 bytes per frame).
    if payload.len() == frame_count * 5 {
        let mut br = BitReader::from_slice(payload);
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let ix = br.read_u32(10)? as f32 / 1023.0;
            let iy = br.read_u32(10)? as f32 / 1023.0;
            let iz = br.read_u32(10)? as f32 / 1023.0;
            let iw = br.read_u32(10)? as f32 / 1023.0;
            let x = defaults[0].x + (defaults[1].x - defaults[0].x) * ix;
            let y = defaults[0].y + (defaults[1].y - defaults[0].y) * iy;
            let z = defaults[0].z + (defaults[1].z - defaults[0].z) * iz;
            let w = defaults[0].w + (defaults[1].w - defaults[0].w) * iw;
            let len2 = x * x + y * y + z * z + w * w;
            let inv = if len2 > 0.0 { 1.0 / len2.sqrt() } else { 1.0 };
            frames.push(Vector4 {
                x: x * inv,
                y: y * inv,
                z: z * inv,
                w: w * inv,
            });
        }
        return Ok(frames);
    }

    // Variant B: residual/kernel (single block, comp_bits=4)
    if frame_count > 34 {
        return Err(error::Error::InvalidData);
    }
    let base_scale = defaults[0].x;
    let block_len = frame_count.saturating_sub(1).max(1);
    let mut frames = Vec::with_capacity(frame_count);
    for local in 0..frame_count {
        let t = local as f32 / block_len as f32;
        let k = Vector4 {
            x: defaults[0].x + (defaults[1].x - defaults[0].x) * t,
            y: defaults[0].y + (defaults[1].y - defaults[0].y) * t,
            z: defaults[0].z + (defaults[1].z - defaults[0].z) * t,
            w: defaults[0].w + (defaults[1].w - defaults[0].w) * t,
        };
        let (r_vec, _) = decode_residual_vector(bytes, pos, base_scale, local, 4, block_len)?;
        let mut q = Vector4 {
            x: k.x + r_vec[0],
            y: k.y + r_vec[1],
            z: k.z + r_vec[2],
            w: k.w + r_vec[3],
        };
        let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
        if len2 > 0.0 {
            let inv = 1.0 / len2.sqrt();
            q.x *= inv;
            q.y *= inv;
            q.z *= inv;
            q.w *= inv;
        }
        frames.push(q);
    }
    Ok(frames)
}

// ========================== 0x4309 Rotate (Inferred Layout) ==========================

/// Decode Anim v1.2 Rotate buffer with header 0x4309.
///
/// **KNOWN ISSUE**: This format uses an inferred layout for endpoints and residual stream.
/// The inference logic attempts to determine the correct endpoint encoding (vec4 f32 vs vec3 f32)
/// and residual stream parameters (comp_bits, padding) by scanning for the best-fit layout.
/// However, this approach is not guaranteed to produce correct results for all files,
/// as the format lacks explicit metadata to disambiguate between multiple valid interpretations.
/// Use with caution and verify results against known-good reference data.
///
/// Layout (inferred):
/// - u32 header (0x4309)
/// - u32 key_count
/// - f32 unk1 (often 1.0)
/// - key_count * u8 frame_indices
/// - align4
/// - f32 base_scale
/// - u16 flags
/// - u16 bits
/// - u32 unk_u32
/// - endpoints: (block_count(key_count) + 1) * Vector4<f32> OR Vector3<f32>
/// - optional padding (0-32 bytes)
/// - residual stream (variable, 4-byte aligned)
///
/// Rebuild key quaternions:
/// - K(local) from endpoint lerp within each 33-key block
/// - R(local) from decode_residual_vector
/// - q_key = normalize(K + R)
/// Then expand to per-frame by nlerp between sparse keyframes using frame_indices.
pub fn decode_rotate_4309(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    let header = read_u32_le(bytes, 0)?;
    if header != 0x4309 {
        return Err(error::Error::InvalidData);
    }

    let key_count = read_u32_le(bytes, 4)? as usize;
    // let _unk1 = read_f32_le(bytes, 8)?;

    if key_count == 0 {
        return Ok(vec![Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 }]);
    }

    let mut pos = 12;
    let end = pos + key_count;
    if end > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let frame_indices_u8: Vec<u8> = bytes[pos..end].to_vec();
    pos = align_up(end, 4);

    if pos + 12 > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, pos)?;
    // let flags = read_u16_le(bytes, pos + 4)?;
    // let bits = read_u16_le(bytes, pos + 6)?;
    // let unk_u32 = read_u32_le(bytes, pos + 8)?;
    pos += 12;

    let endpoint_count = compute_block_count_4309(key_count) + 1;
    let (endpoints, residual_payload, comp_bits, q_counts) =
        infer_4309_layout(bytes, pos, endpoint_count, base_scale, key_count)?;

    let mut prefix_words = vec![0usize];
    for &q in &q_counts {
        prefix_words.push(prefix_words.last().unwrap() + q);
    }

    let mut key_quats = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let block_len = compute_block_len_4309(key_count, block_idx).max(1);

        let t_block = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let k = Vector4 {
            x: e0.x + t_block * (e1.x - e0.x),
            y: e0.y + t_block * (e1.y - e0.y),
            z: e0.z + t_block * (e1.z - e0.z),
            w: e0.w + t_block * (e1.w - e0.w),
        };

        let rs = 4 * prefix_words[block_idx];
        let (r_vec, _) = decode_residual_vector(&residual_payload, rs, base_scale, local, comp_bits, block_len)?;
        let qx = k.x + r_vec[0];
        let qy = k.y + r_vec[1];
        let qz = k.z + r_vec[2];
        let qw = k.w + r_vec[3];
        key_quats.push(quat_normalize(qx, qy, qz, qw));
    }

    let last_frame = *frame_indices_u8.iter().max().unwrap_or(&0) as usize;
    let total_frames = last_frame + 1;
    let mut frames_q = vec![key_quats[0]; total_frames];

    let first_f = frame_indices_u8[0] as usize;
    for f in 0..first_f.min(total_frames) {
        frames_q[f] = key_quats[0];
    }

    for i in 0..frame_indices_u8.len() - 1 {
        let f0 = frame_indices_u8[i] as usize;
        let f1 = frame_indices_u8[i + 1] as usize;
        if f1 <= f0 {
            continue;
        }
        let span = f1 - f0;
        if span == 0 {
            if f0 < total_frames {
                frames_q[f0] = key_quats[i];
            }
            continue;
        }
        for f in f0..=(f1.min(total_frames - 1)) {
            let t = (f - f0) as f32 / span as f32;
            frames_q[f] = quat_nlerp(key_quats[i], key_quats[i + 1], t);
        }
    }

    let last_f = frame_indices_u8[frame_indices_u8.len() - 1] as usize;
    for f in last_f..total_frames {
        frames_q[f] = key_quats[key_quats.len() - 1];
    }

    Ok(frames_q)
}

fn compute_block_count_4309(key_count: usize) -> usize {
    if key_count <= 1 {
        1
    } else {
        (key_count - 1) / 33 + 1
    }
}

fn compute_block_len_4309(key_count: usize, block_idx: usize) -> usize {
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

fn compute_block_qcounts_4309(
    payload: &[u8],
    residual_start0: usize,
    base_scale: f32,
    comp_bits: usize,
    key_count: usize,
) -> Result<Vec<usize>, error::Error> {
    if key_count <= 1 {
        return Ok(vec![0]);
    }

    let last_block = (key_count - 1) / 33;
    let blocks = last_block + 1;

    let mut q_counts = Vec::with_capacity(blocks);
    let mut cursor = residual_start0;
    for block_idx in 0..blocks {
        let block_len = compute_block_len_4309(key_count, block_idx);
        if block_len <= 1 {
            q_counts.push(0);
            continue;
        }

        let (_, end_off) = decode_residual_vector(payload, cursor, base_scale, 1, comp_bits, block_len)?;
        let delta = end_off - cursor;
        if delta == 0 || (delta % 4) != 0 {
            return Err(error::Error::InvalidData);
        }
        q_counts.push(delta / 4);
        cursor = end_off;
    }
    Ok(q_counts)
}

fn try_pick_comp_bits_4309(
    payload: &[u8],
    residual_start0: usize,
    base_scale: f32,
    key_count: usize,
) -> Result<(usize, Vec<usize>, usize), error::Error> {
    let mut best: Option<(usize, Vec<usize>, usize)> = None;
    let mut best_key: Option<(usize, i32)> = None;

    for comp_bits in [4, 3, 2, 1] {
        let q_counts = match compute_block_qcounts_4309(payload, residual_start0, base_scale, comp_bits, key_count) {
            Ok(q) => q,
            Err(_) => continue,
        };

        let sum: usize = q_counts.iter().sum();
        let end_off = residual_start0 + 4 * sum;
        if end_off > payload.len() {
            continue;
        }
        let slack = payload.len() - end_off;
        let cand_key = (slack, -(comp_bits as i32));
        if best_key.is_none() || cand_key < best_key.unwrap() {
            best = Some((comp_bits, q_counts, end_off));
            best_key = Some(cand_key);
        }
    }

    best.ok_or(error::Error::InvalidData)
}

fn try_parse_endpoints_4309(
    curve_bytes: &[u8],
    endpoints_off: usize,
    endpoint_count: usize,
    elem_size: usize,
) -> Result<Vec<Vector4>, error::Error> {
    let end = endpoints_off + endpoint_count * elem_size;
    if endpoints_off >= curve_bytes.len() || end > curve_bytes.len() {
        return Err(error::Error::InvalidData);
    }
    if (endpoints_off % 4) != 0 {
        return Err(error::Error::InvalidData);
    }

    let mut out = Vec::with_capacity(endpoint_count);
    let mut max_abs = 0.0f32;

    if elem_size == 16 {
        for i in 0..endpoint_count {
            let q = read_vec4_f32_le(curve_bytes, endpoints_off + i * 16)?;
            max_abs = max_abs.max(q.x.abs()).max(q.y.abs()).max(q.z.abs()).max(q.w.abs());
            out.push(quat_normalize(q.x, q.y, q.z, q.w));
        }
    } else if elem_size == 12 {
        for i in 0..endpoint_count {
            let v = read_vec3_f32_le(curve_bytes, endpoints_off + i * 12)?;
            max_abs = max_abs.max(v.x.abs()).max(v.y.abs()).max(v.z.abs());
            out.push(reconstruct_quat_from_xyz(v.x, v.y, v.z, 1.0));
        }
    } else {
        return Err(error::Error::InvalidData);
    }

    if !max_abs.is_finite() || max_abs > 1.0e6 {
        return Err(error::Error::InvalidData);
    }

    // Enforce sign continuity between endpoints to reduce discontinuities.
    for i in 1..out.len() {
        if quat_dot(out[i - 1], out[i]) < 0.0 {
            out[i] = quat_neg(out[i]);
        }
    }

    Ok(out)
}

fn infer_4309_layout(
    curve_bytes: &[u8],
    base_offset: usize,
    endpoint_count: usize,
    base_scale: f32,
    key_count: usize,
) -> Result<(Vec<Vector4>, Vec<u8>, usize, Vec<usize>), error::Error> {
    let mut best: Option<(Vec<Vector4>, Vec<u8>, usize, Vec<usize>)> = None;
    let mut best_key: Option<(usize, i32, usize, usize)> = None;

    for elem_size in [16, 12] {
        let endpoints = match try_parse_endpoints_4309(curve_bytes, base_offset, endpoint_count, elem_size) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let endpoints_end = base_offset + endpoint_count * elem_size;
        for pad in (0..=0x20).step_by(4) {
            let residual_off = endpoints_end + pad;
            if residual_off >= curve_bytes.len() {
                continue;
            }
            let residual_payload = curve_bytes[residual_off..].to_vec();
            if (residual_payload.len() % 4) != 0 {
                continue;
            }

            let (comp_bits, q_counts, end_off) = match try_pick_comp_bits_4309(&residual_payload, 0, base_scale, key_count) {
                Ok(r) => r,
                Err(_) => continue,
            };

            let slack = curve_bytes.len() - (residual_off + end_off);
            let elem_rank = if elem_size == 16 { 0 } else { 1 };
            let cand_key = (slack, -(comp_bits as i32), elem_rank, residual_off);
            if best_key.is_none() || cand_key < best_key.unwrap() {
                best = Some((endpoints.clone(), residual_payload.clone(), comp_bits, q_counts.clone()));
                best_key = Some(cand_key);
            }
            if slack <= 16 {
                break;
            }
        }
    }

    best.ok_or(error::Error::InvalidData)
}

fn quat_normalize(x: f32, y: f32, z: f32, w: f32) -> Vector4 {
    let n2 = x * x + y * y + z * z + w * w;
    if n2 <= 0.0 {
        return Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };
    }
    let inv = 1.0 / n2.sqrt();
    Vector4 {
        x: x * inv,
        y: y * inv,
        z: z * inv,
        w: w * inv,
    }
}

fn quat_nlerp(q0: Vector4, q1: Vector4, t: f32) -> Vector4 {
    let x = q0.x + (q1.x - q0.x) * t;
    let y = q0.y + (q1.y - q0.y) * t;
    let z = q0.z + (q1.z - q0.z) * t;
    let w = q0.w + (q1.w - q0.w) * t;
    quat_normalize(x, y, z, w)
}

fn quat_dot(a: Vector4, b: Vector4) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w
}

fn quat_neg(q: Vector4) -> Vector4 {
    Vector4 {
        x: -q.x,
        y: -q.y,
        z: -q.z,
        w: -q.w,
    }
}

fn reconstruct_quat_from_xyz(x: f32, y: f32, z: f32, sign: f32) -> Vector4 {
    let s = x * x + y * y + z * z;
    let w = sign * (1.0 - s).max(0.0).sqrt();
    quat_normalize(x, y, z, w)
}

// ========================== 0x3409 Translate (Dynamic Block + Residual) ==========================

/// Decode Anim v1.2 Translate buffer with header 0x3409.
///
/// Layout (inferred):
/// - u32 header (0x3409)
/// - u32 key_count (frames)
/// - f32 unk1 (often 1.0)
/// - f32 base_scale
/// - u16 flags
/// - u16 bits
/// - endpoints: (block_count(key_count) + 1) * Vector3<f32> OR compressed format
/// - residual stream (variable, 4-byte aligned)
///
/// Rebuild key values:
/// - K(local) from endpoint lerp within each 33-key block
/// - R(local) from decode_residual_vector
/// - value_key = K + R
pub fn decode_translate_3409(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    if bytes.len() < 20 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3409 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let _unk1 = read_f32_le(bytes, 8)?;
    let base_scale = read_f32_le(bytes, 12)?;
    let _flags = read_u16_le(bytes, 16)?;
    let _bits = read_u16_le(bytes, 18)?;

    if key_count == 0 {
        return Ok(vec![Vector3 { x: 0.0, y: 0.0, z: 0.0 }]);
    }

    let blocks = compute_block_count(key_count);
    let endpoint_count = blocks + 1;

    // Infer layout: try different endpoint encodings and residual offsets
    let (endpoints, comp_bits, residual_off, _q_counts) = 
        infer_3409_layout(bytes, 20, endpoint_count, base_scale, key_count)?;

    // Compute prefix words for each block
    let q_counts = compute_block_qcounts(bytes, residual_off, base_scale, comp_bits, key_count)?;
    let mut prefix_words = vec![0usize];
    for q in &q_counts {
        prefix_words.push(prefix_words.last().copied().unwrap_or(0) + *q);
    }

    // Decode all keys
    let mut out = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let mut block_len = compute_block_len(key_count, block_idx);
        if block_len == 0 {
            block_len = 1;
        }
        let t_block = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let kx = e0.x + (e1.x - e0.x) * t_block;
        let ky = e0.y + (e1.y - e0.y) * t_block;
        let kz = e0.z + (e1.z - e0.z) * t_block;
        let rs = residual_off + 4 * prefix_words[block_idx];
        let (r_vec, _) = decode_residual_vector(bytes, rs, base_scale, local, comp_bits, block_len)?;
        out.push(Vector3 {
            x: kx + r_vec.get(0).copied().unwrap_or(0.0),
            y: ky + r_vec.get(1).copied().unwrap_or(0.0),
            z: kz + r_vec.get(2).copied().unwrap_or(0.0),
        });
    }
    Ok(out)
}

fn infer_3409_layout(
    bytes: &[u8],
    scan_start: usize,
    endpoint_count: usize,
    base_scale: f32,
    key_count: usize,
) -> Result<(Vec<Vector3>, usize, usize, Vec<usize>), error::Error> {
    let mut best: Option<(Vec<Vector3>, usize, usize, Vec<usize>)> = None;
    let mut best_key: Option<(usize, i32, usize)> = None;

    // Try float3 (12 bytes) and short4 (8 bytes) encodings
    for elem_size in [12, 8] {
        let endpoint_base = scan_start;
        let endpoints_end = endpoint_base + endpoint_count * elem_size;
        if endpoints_end > bytes.len() {
            continue;
        }

        let endpoints = match try_parse_endpoints_3409(bytes, endpoint_base, endpoint_count, elem_size) {
            Ok(e) => e,
            Err(_) => continue,
        };

        // Try different residual offsets (with padding)
        for pad in (0..=32).step_by(4) {
            let residual_off = endpoints_end + pad;
            if residual_off >= bytes.len() {
                continue;
            }

            // Try different component counts
            for comp_bits in [3, 2, 1, 4] {
                let q_counts = match compute_block_qcounts(bytes, residual_off, base_scale, comp_bits, key_count) {
                    Ok(q) => q,
                    Err(_) => continue,
                };
                let end_off = residual_off + 4 * q_counts.iter().sum::<usize>();
                if end_off > bytes.len() {
                    continue;
                }
                let slack = bytes.len() - end_off;
                let cand_key = (slack, -(comp_bits as i32), residual_off);
                if best.is_none() || cand_key < best_key.unwrap() {
                    best = Some((endpoints.clone(), comp_bits, residual_off, q_counts));
                    best_key = Some(cand_key);
                }
                break;
            }
        }
    }

    best.ok_or(error::Error::InvalidData)
}

fn try_parse_endpoints_3409(
    bytes: &[u8],
    endpoint_base: usize,
    endpoint_count: usize,
    elem_size: usize,
) -> Result<Vec<Vector3>, error::Error> {
    let end = endpoint_base + endpoint_count * elem_size;
    if end > bytes.len() {
        return Err(error::Error::InvalidData);
    }

    let mut out = Vec::with_capacity(endpoint_count);
    let mut max_abs = 0.0f32;

    if elem_size == 12 {
        for i in 0..endpoint_count {
            let v = read_vec3_f32_le(bytes, endpoint_base + i * 12)?;
            max_abs = max_abs.max(v.x.abs()).max(v.y.abs()).max(v.z.abs());
            out.push(v);
        }
    } else if elem_size == 8 {
        // short4 compressed format
        for i in 0..endpoint_count {
            let o = endpoint_base + i * 8;
            let x = i16::from_le_bytes([bytes[o], bytes[o + 1]]) as f32 * G_CURVE_SHORT4_SCALE;
            let y = i16::from_le_bytes([bytes[o + 2], bytes[o + 3]]) as f32 * G_CURVE_SHORT4_SCALE;
            let z = i16::from_le_bytes([bytes[o + 4], bytes[o + 5]]) as f32 * G_CURVE_SHORT4_SCALE;
            max_abs = max_abs.max(x.abs()).max(y.abs()).max(z.abs());
            out.push(Vector3 { x, y, z });
        }
    } else {
        return Err(error::Error::InvalidData);
    }

    if !max_abs.is_finite() || max_abs > 1.0e6 {
        return Err(error::Error::InvalidData);
    }

    Ok(out)
}

// ========================== 0x4409 Rotate (Dynamic Block + Residual) ==========================

/// Decode Anim v1.2 Rotate buffer with header 0x4409.
///
/// Layout (curve-level 0x3409/0x4409 family):
/// - u32 header (0x4409)
/// - u32 key_count (frames)
/// - f32 unk1 (often 1.0)
/// - f32 base_scale
/// - u16 flags (semantics still unclear; do not treat as block count)
/// - u16 bits
/// - endpoints: (block_count(key_count) + 1) * Vector4<f32>
/// - residual stream (variable, 4-byte aligned)
///
/// Some files appear to contain an extra u32 immediately after `bits`, shifting the endpoints base
/// from 0x14 to 0x18. This decoder tries both endpoint bases (0x14 and 0x18) and validates the
/// candidate by walking the residual stream.
///
/// Rebuild key quaternions:
/// - K(local) from endpoint lerp within each 33-key block
/// - R(local) from decode_residual_vector
/// - q_key = normalize(K + R)
pub fn decode_rotate_4409(bytes: &[u8]) -> Result<Vec<Vector4>, error::Error> {
    if bytes.len() < 0x14 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4409 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let _unk1 = read_f32_le(bytes, 8)?;
    let base_scale = read_f32_le(bytes, 12)?;
    let _bits = read_u16_le(bytes, 18)?;

    if key_count == 0 {
        return Ok(vec![Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 }]);
    }

    let block_count = if key_count <= 1 {
        0usize
    } else {
        (key_count - 1) / 33 + 1
    };
    let endpoint_count = block_count + 1;

    fn walk_residual_4409(
        payload: &[u8],
        residual_off: usize,
        base_scale: f32,
        comp_bits: usize,
        key_count: usize,
        block_count: usize,
    ) -> Result<usize, error::Error> {
        let mut cursor = residual_off;
        for block_idx in 0..block_count {
            let block_len = compute_block_len(key_count, block_idx);
            if block_len <= 1 {
                continue;
            }
            let (_, end_off) = decode_residual_vector(payload, cursor, base_scale, 1, comp_bits, block_len)?;
            let delta = end_off.saturating_sub(cursor);
            if delta == 0 || (delta % 4) != 0 {
                return Err(error::Error::InvalidData);
            }
            cursor = end_off;
        }
        Ok(cursor)
    }

    let mut best: Option<(Vec<Vector4>, usize, usize)> = None; // (endpoints, residual_off, comp_bits)
    let mut best_key: Option<(usize, i32, usize)> = None; // (slack, -comp_bits, residual_off)

    for endpoint_base in [0x14usize, 0x18usize] {
        let endpoints_size = endpoint_count * 16;
        let residual_off = endpoint_base + endpoints_size;
        if residual_off > bytes.len() {
            continue;
        }
        if (endpoint_base % 4) != 0 || (residual_off % 4) != 0 {
            continue;
        }

        let mut endpoints = Vec::with_capacity(endpoint_count);
        let mut max_abs = 0.0f32;
        let mut ok = true;
        for i in 0..endpoint_count {
            let q = match read_vec4_f32_le(bytes, endpoint_base + i * 16) {
                Ok(v) => v,
                Err(_) => {
                    ok = false;
                    break;
                }
            };
            max_abs = max_abs.max(q.x.abs()).max(q.y.abs()).max(q.z.abs()).max(q.w.abs());
            endpoints.push(q);
        }
        if !ok || !max_abs.is_finite() || max_abs > 1.0e6 {
            continue;
        }

        for comp_bits in [4usize, 3, 2, 1] {
            let end_off = match walk_residual_4409(bytes, residual_off, base_scale, comp_bits, key_count, block_count)
            {
                Ok(v) => v,
                Err(_) => continue,
            };
            if end_off > bytes.len() {
                continue;
            }
            let slack = bytes.len() - end_off;
            let cand_key = (slack, -(comp_bits as i32), residual_off);
            if best_key.is_none() || cand_key < best_key.unwrap() {
                best = Some((endpoints.clone(), residual_off, comp_bits));
                best_key = Some(cand_key);
            }
            break;
        }
    }

    let (endpoints, residual_off, comp_bits) = best.ok_or(error::Error::InvalidData)?;

    // Compute prefix words for each block
    let q_counts = compute_block_qcounts(bytes, residual_off, base_scale, comp_bits, key_count)?;
    let mut prefix_words = vec![0usize];
    for q in &q_counts {
        prefix_words.push(prefix_words.last().copied().unwrap_or(0) + *q);
    }

    // Decode all keys
    let mut out = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = key_idx / 33;
        let local = key_idx - 33 * block_idx;
        let mut block_len = compute_block_len(key_count, block_idx);
        if block_len == 0 {
            block_len = 1;
        }
        let t_block = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let k = Vector4 {
            x: e0.x + (e1.x - e0.x) * t_block,
            y: e0.y + (e1.y - e0.y) * t_block,
            z: e0.z + (e1.z - e0.z) * t_block,
            w: e0.w + (e1.w - e0.w) * t_block,
        };
        let rs = residual_off + 4 * prefix_words[block_idx];
        let (r_vec, _) = decode_residual_vector(bytes, rs, base_scale, local, comp_bits, block_len)?;
        let mut q = Vector4 {
            x: k.x + r_vec.get(0).copied().unwrap_or(0.0),
            y: k.y + r_vec.get(1).copied().unwrap_or(0.0),
            z: k.z + r_vec.get(2).copied().unwrap_or(0.0),
            w: k.w + r_vec.get(3).copied().unwrap_or(0.0),
        };
        let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
        if len2 > 0.0 {
            let inv = 1.0 / len2.sqrt();
            q.x *= inv;
            q.y *= inv;
            q.z *= inv;
            q.w *= inv;
        }
        out.push(q);
    }
    Ok(out)
}

// ========================== Scale Decoders (Aliases to Translate) ==========================

/// Scale uses the same encoding as Translate for all variants.
pub fn decode_scale_3200(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3200(bytes)
}

pub fn decode_scale_3208(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3208(bytes)
}

pub fn decode_scale_3209(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3209(bytes)
}

pub fn decode_scale_3300(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3300(bytes)
}

pub fn decode_scale_3308(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3308(bytes)
}

pub fn decode_scale_3309(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3309(bytes)
}

pub fn decode_scale_3400(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3400(bytes)
}

pub fn decode_scale_3408(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3408(bytes)
}

pub fn decode_scale_3409(bytes: &[u8]) -> Result<Vec<Vector3>, error::Error> {
    decode_translate_3409(bytes)
}

// ----------------------------- Tests -----------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_row_matches_formula() {
        let cache = kernel();
        // v18 = 1 -> dim = 4, row 0, col 0
        let first = cache.row(1, 0)[0];
        let expected = ((0.5_f32) * (0.5_f32 * (PI / 4.0))).cos() * (2.0_f32 / 4.0).sqrt();
        assert!((first - expected).abs() < 1e-6, "first={}, expected={}", first, expected);
        // spot check another entry for stability
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
        // Construct a minimal residual stream:
        // word0=0x4000, word1=0x8000, byte4=255, byte5=0x10 (v14=1),
        // byte6=0x00, byte7=0x00 so v18=1 and weights use scaled_slope.
        let mut residual = Vec::new();
        residual.extend_from_slice(&0x4000u16.to_le_bytes()); // word0
        residual.extend_from_slice(&0x8000u16.to_le_bytes()); // word1
        residual.push(255); // byte4
        residual.push(0x10); // byte5: v14=1, v15=0
        residual.push(0x00); // byte6: v12=0, v13=0
        residual.push(0x00); // byte7: v16=0, v17=0
        residual.extend_from_slice(&[0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // one short4 block

        let base_scale = 1.0;
        let local_idx = 1;
        let block_len = 4;
        let (value, next) =
            decode_residual_component(&residual, 0, base_scale, local_idx, block_len).unwrap();

        // Cursor should advance past header (8) + one short4 (8) = 16 bytes.
        assert_eq!(next, 16);
        // Value should be non-zero and within expected range.
        assert!(value.abs() > 0.0);
        assert!(value.abs() < 1.0);
    }
}
