use glam::{Quat, quat};

use super::common::{
    align_up, block_index_for_key, compute_block_count_type9, compute_block_len_type9,
    decode_residual_vector, expand_sparse_quat, quat_normalize, read_f32_le, read_u16_le,
    read_u32_le, read_vec3_f32_le, read_vec4_f32_le,
};
use super::translate::read_single_block_header;
use crate::anim_data::{Vector4, error::Error};

pub fn decode_rotate_4300(bytes: &[u8]) -> Result<Vec<Quat>, Error> {
    if bytes.len() < 12 {
        return Err(Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4300 {
        return Err(Error::InvalidData);
    }
    let frame_count = read_u32_le(bytes, 4)? as usize;
    if frame_count == 0 {
        return Ok(vec![Quat::IDENTITY]);
    }

    // Variant A: 16-byte header (frames_per_key + one more f32) + frame_count * vec4<f32>.
    // Variant B: 12-byte header (frames_per_key only) + frame_count * vec4<f32>.
    // Variant C (observed in game data): 12-byte header + u8 frame_indices[frame_count] + align4 + frame_count * vec4<f32>.
    //
    // The same magic (0x4300) is used for these variants, so we must infer the layout from length.

    // Prefer the indexed-key variant if it matches exactly.
    let indexed_payload_off = align_up(12 + frame_count, 4);
    if indexed_payload_off <= bytes.len() && indexed_payload_off + frame_count * 16 == bytes.len() {
        let frames_per_key = read_f32_le(bytes, 8)?;
        let frame_indices: Vec<usize> = bytes[12..12 + frame_count]
            .iter()
            .map(|index| (*index as f32 * frames_per_key).round() as usize)
            .collect();
        let mut key_vals = Vec::with_capacity(frame_count);
        let mut pos = indexed_payload_off;
        for _ in 0..frame_count {
            let mut q = Quat::from_array(read_vec4_f32_le(bytes, pos)?.to_array());
            pos += 16;
            let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
            if len2 > 0.0 {
                let inv = 1.0 / len2.sqrt();
                q.x *= inv;
                q.y *= inv;
                q.z *= inv;
                q.w *= inv;
            } else {
                q = Quat::IDENTITY;
            }
            key_vals.push((q.x, q.y, q.z, q.w));
        }
        let total_frames = frame_indices.iter().copied().max().unwrap_or(0) + 1;
        return Ok(expand_sparse_quat(&frame_indices, &key_vals, total_frames));
    }

    // Fallback to raw stream variants.
    let pos = if bytes.len() >= 16 + frame_count * 16 && bytes.len() != 12 + frame_count * 16 {
        16
    } else {
        12
    };
    if pos + frame_count * 16 > bytes.len() {
        return Err(Error::InvalidData);
    }
    let mut frames = Vec::with_capacity(frame_count);
    let mut pos = pos;
    for _ in 0..frame_count {
        let mut q = Quat::from_array(read_vec4_f32_le(bytes, pos)?.to_array());
        pos += 16;
        let len2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
        if len2 > 0.0 {
            let inv = 1.0 / len2.sqrt();
            q.x *= inv;
            q.y *= inv;
            q.z *= inv;
            q.w *= inv;
        } else {
            q = Quat::IDENTITY;
        }
        frames.push(q);
    }
    Ok(frames)
}

pub fn decode_rotate_4400(bytes: &[u8]) -> Result<Vec<Quat>, Error> {
    if bytes.len() < 12 {
        return Err(Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4400 {
        return Err(Error::InvalidData);
    }
    let frame_count = read_u32_le(bytes, 4)? as usize;
    let mut pos = 12;
    let mut frames = Vec::with_capacity(frame_count);
    for i in 0..frame_count {
        let mut q = if pos + 16 <= bytes.len() {
            Quat::from_array(read_vec4_f32_le(bytes, pos)?.to_array())
        } else if pos + 12 <= bytes.len() && i + 1 == frame_count {
            let v = read_vec3_f32_le(bytes, pos)?;
            // TODO: is this the correct w component?
            quat(v.x, v.y, v.z, 1.0)
        } else {
            return Err(Error::InvalidData);
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
            q = Quat::IDENTITY;
        }
        frames.push(q);
    }
    Ok(frames)
}

pub fn decode_rotate_4200(bytes: &[u8]) -> Result<Vec<Quat>, Error> {
    if bytes.len() < 12 {
        return Err(Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4200 {
        return Err(Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    if key_count == 0 {
        return Ok(vec![Quat::IDENTITY]);
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
        &key_vals
            .iter()
            .map(|q| (q.x, q.y, q.z, q.w))
            .collect::<Vec<_>>(),
        total_frames,
    ))
}

pub fn decode_rotate_4208(bytes: &[u8]) -> Result<Vec<Quat>, Error> {
    if bytes.len() < 16 {
        return Err(Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4208 {
        return Err(Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let frames_per_key = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Quat::IDENTITY]);
    }
    // Single residual block with two quaternion endpoints. VS2 uses key_count=34
    // (33+1); residual block_len = key_count - 1 (same pattern as 0x3308).
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * frames_per_key).round() as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    let base_scale = read_f32_le(bytes, pos)?;
    let quat0 = read_vec4_f32_le(bytes, pos + 4)?;
    let quat1 = read_vec4_f32_le(bytes, pos + 20)?;
    let residual_off = pos + 36;
    if residual_off > bytes.len() || residual_off % 4 != 0 {
        return Err(Error::InvalidData);
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
        let (r_vec, _) =
            decode_residual_vector(bytes, residual_off, base_scale, local, 4, block_len)?;
        let mut q = Vector4 {
            x: k.x + r_vec.first().copied().unwrap_or(0.0),
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
        key_quats.push((q.x, q.y, q.z, q.w));
    }
    let total_frames = frame_indices.iter().copied().max().unwrap_or(0) + 1;
    Ok(expand_sparse_quat(&frame_indices, &key_quats, total_frames))
}

#[cfg(test)]
mod tests_4208 {
    use super::*;

    #[test]
    fn decode_vs2_4208_key_count_34() {
        let bytes = include_bytes!("fixtures/glshot_spread_4208_b191.bin");
        let frames = decode_rotate_4208(bytes).expect("0x4208 k=34");
        assert!(frames.len() >= 34);
        for q in &frames {
            assert!(q.x.is_finite() && q.y.is_finite() && q.z.is_finite() && q.w.is_finite());
        }
    }
}

pub fn decode_rotate_4209(bytes: &[u8]) -> Result<Vec<Quat>, Error> {
    if bytes.len() < 16 {
        return Err(Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4209 {
        return Err(Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let frames_per_key = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Quat::IDENTITY]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * frames_per_key).round() as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    if pos + 8 > bytes.len() {
        return Err(Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, pos)?;
    let block_count = read_u16_le(bytes, pos + 4)? as usize;
    let expected_blocks = compute_block_count_type9(key_count);
    if block_count != expected_blocks || block_count == 0 {
        return Err(Error::InvalidData);
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
        return Err(Error::InvalidData);
    }
    let mut endpoints = Vec::with_capacity(endpoint_count);
    for i in 0..endpoint_count {
        endpoints.push(read_vec4_f32_le(bytes, endpoint_base + i * 16)?);
    }
    let residual_off = endpoint_base + endpoints_size;
    if residual_off > bytes.len() || !residual_off.is_multiple_of(4) {
        return Err(Error::InvalidData);
    }
    let mut residual_starts = Vec::with_capacity(block_count);
    residual_starts.push(residual_off);
    for w in &block_words {
        residual_starts.push(residual_off + 4 * *w);
    }
    let mut key_quats = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = block_index_for_key(key_idx, block_count);
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

pub fn decode_rotate_4308(bytes: &[u8]) -> Result<Vec<Quat>, Error> {
    if bytes.len() < 12 {
        return Err(Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4308 {
        return Err(Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    // Frames per stored key; see `common::read_blocked_header`.
    let frames_per_key = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Quat::IDENTITY]);
    }
    let mut pos = 12;
    if pos + key_count > bytes.len() {
        return Err(Error::InvalidData);
    }
    let frame_indices: Vec<usize> = bytes[pos..pos + key_count]
        .iter()
        .map(|index| (*index as f32 * frames_per_key).round() as usize)
        .collect();
    pos = align_up(pos + key_count, 4);
    if pos + 36 > bytes.len() {
        return Err(Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, pos)?;
    let quat0 = read_vec4_f32_le(bytes, pos + 4)?;
    let quat1 = read_vec4_f32_le(bytes, pos + 20)?;
    pos += 36;

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

/// Decode a `0x4408` rotation curve: one quaternion per frame over a single
/// residual block.
///
/// Layout: `magic | frame_count | frames_per_key | base_scale | endpoint0 | endpoint1 | residual`,
/// matching `0x3408` with 4-component endpoints. `base_scale` is the f32 at
/// offset 12, exactly as in the multi-block `0x4409` sibling.
pub fn decode_rotate_4408(bytes: &[u8]) -> Result<Vec<Quat>, Error> {
    let (base_scale, endpoints, residual_off, frame_count) =
        read_single_block_header(bytes, 0x4408)?;
    let e0 = read_vec4_f32_le(bytes, endpoints)?;
    let e1 = read_vec4_f32_le(bytes, endpoints + 16)?;

    let block_len = frame_count.saturating_sub(1).max(1);
    let mut frames = Vec::with_capacity(frame_count);
    for local in 0..frame_count {
        let t = local as f32 / block_len as f32;
        let (residual, _) =
            decode_residual_vector(bytes, residual_off, base_scale, local, 4, block_len)?;
        frames.push(quat_normalize(
            e0.x + (e1.x - e0.x) * t + residual[0],
            e0.y + (e1.y - e0.y) * t + residual[1],
            e0.z + (e1.z - e0.z) * t + residual[2],
            e0.w + (e1.w - e0.w) * t + residual[3],
        ));
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EXVS2 `001hito_028gunwtv_..._stepend_stk_air_bk` SAKOTSU_R.
    ///
    /// `0x4408` takes `base_scale` from offset 12, like its `0x4409` sibling. The
    /// previous decoder used `endpoint0.x` instead, which for this clavicle is
    /// -0.584 against a real scale of 0.0056 — a 104x residual amplification that
    /// turned a 0.5 deg/frame curve into a 27 deg/frame flicker.
    #[test]
    fn decode_exvs2_gunwtv_4408_base_scale_from_offset_12() {
        let bytes = include_bytes!("fixtures/gunwtv_sakotsu_r_4408.bin");
        let base_scale = f32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let endpoint0_x = f32::from_le_bytes(bytes[16..20].try_into().unwrap());
        assert!((base_scale - 0.005615).abs() < 1e-6);
        assert!((endpoint0_x + 0.584274).abs() < 1e-5);

        let frames = decode_rotate_4408(bytes).expect("0x4408 must decode");
        assert_eq!(15, frames.len());

        let endpoint0 = Quat::from_xyzw(
            endpoint0_x,
            f32::from_le_bytes(bytes[20..24].try_into().unwrap()),
            f32::from_le_bytes(bytes[24..28].try_into().unwrap()),
            f32::from_le_bytes(bytes[28..32].try_into().unwrap()),
        );
        assert!((frames[0].dot(endpoint0).abs() - 1.0).abs() < 1e-4);

        let worst = frames
            .windows(2)
            .map(|w| 2.0 * w[0].dot(w[1]).abs().min(1.0).acos().to_degrees())
            .fold(0.0f32, f32::max);
        assert!(worst < 2.0, "max per-frame rotation step {worst} deg");
    }

    /// The 0x_408 family is single-block, so it never stores more than 34 keys.
    #[test]
    fn decode_rotate_4408_rejects_multi_block_key_count() {
        let mut bytes = include_bytes!("fixtures/gunwtv_sakotsu_r_4408.bin").to_vec();
        bytes[4..8].copy_from_slice(&100u32.to_le_bytes());
        assert!(decode_rotate_4408(&bytes).is_err());
    }
}
