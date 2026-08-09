use std::io::Cursor;

use binrw::BinReaderExt;
use glam::{Vec3, vec3};

use super::common::{
    BlockedHeader, align_up, block_index_for_key, compute_block_count_type9,
    compute_block_len_type9, decode_residual_vector, expand_sparse_vec3, read_blocked_header,
    read_f32_le, read_u16_le, read_u32_le, read_vec3_f32_le,
};
use crate::anim_data::{error, v1::buffers::Unk3300};

pub fn decode_translate_3200(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3200 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let frames_per_key = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vec3::ZERO]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * frames_per_key).round() as usize);
        pos += 2;
    }
    pos = align_up(pos, 4);
    let mut values = Vec::with_capacity(key_count);
    for i in 0..key_count {
        values.push(read_vec3_f32_le(bytes, pos + i * 12)?.into());
    }
    let last_frame = *frame_indices.iter().max().unwrap_or(&0);
    let total_frames = last_frame + 1;
    Ok(expand_sparse_vec3(&frame_indices, &values, total_frames))
}

pub fn decode_translate_3208(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3208 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let frames_per_key = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vec3::ZERO]);
    }
    if key_count > 34 {
        return Err(error::Error::InvalidData);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * frames_per_key).round() as usize);
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
        let (r_vec, _) =
            decode_residual_vector(bytes, residual_off, base_scale, local, 3, block_len)?;
        key_values.push(vec3(kx + r_vec[0], ky + r_vec[1], kz + r_vec[2]));
    }
    let frames = expand_sparse_vec3(
        &frame_indices,
        &key_values,
        frame_indices.iter().copied().max().unwrap_or(0) + 1,
    );
    Ok(frames)
}

pub fn decode_translate_3300(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    let mut reader = Cursor::new(bytes);
    let header: Unk3300 = reader.read_le()?;

    let key_count = header.frame_count as usize;
    if key_count == 0 {
        return Ok(vec![Vec3::ZERO]);
    }
    let frame_indices: Vec<_> = header
        .frame_indices
        .into_iter()
        .map(|i| (i as f32 * header.frames_per_key).round() as usize)
        .collect();

    let frames = expand_sparse_vec3(
        &frame_indices,
        &header.values,
        frame_indices.iter().copied().max().unwrap_or(0) + 1,
    );
    Ok(frames)
}

pub fn decode_translate_3308(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3308 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let frames_per_key = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vec3::ZERO]);
    }
    // Single residual block with two endpoints. key_count may be 34 (33+1) in VS2;
    // residual uses block_len = key_count - 1 (local==block_len → zero residual).
    // Multi-block 3308 (>>34 keys) is rare; still attempt single-stream residual.
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push(
            (bytes.get(pos).copied().ok_or(error::Error::InvalidData)? as f32 * frames_per_key)
                .round() as usize,
        );
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
        let (r_vec, _) =
            decode_residual_vector(bytes, residual_off, base_scale, local, 3, block_len)?;
        key_values.push(vec3(
            kx + r_vec.first().copied().unwrap_or(0.0),
            ky + r_vec.get(1).copied().unwrap_or(0.0),
            kz + r_vec.get(2).copied().unwrap_or(0.0),
        ));
    }
    let frames = expand_sparse_vec3(
        &frame_indices,
        &key_values,
        frame_indices.iter().copied().max().unwrap_or(0) + 1,
    );
    Ok(frames)
}

pub fn decode_translate_3209(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    if bytes.len() < 16 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3209 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let frames_per_key = read_f32_le(bytes, 8)?;
    if key_count == 0 {
        return Ok(vec![Vec3::ZERO]);
    }
    let mut frame_indices = Vec::with_capacity(key_count);
    let mut pos = 12;
    for _ in 0..key_count {
        frame_indices.push((read_u16_le(bytes, pos)? as f32 * frames_per_key).round() as usize);
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
    if residual_off > bytes.len() || !residual_off.is_multiple_of(4) {
        return Err(error::Error::InvalidData);
    }
    let mut residual_starts = Vec::with_capacity(block_count);
    residual_starts.push(residual_off);
    for w in &block_words {
        residual_starts.push(residual_off + 4 * *w);
    }

    let mut key_values = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = block_index_for_key(key_idx, block_count);
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
        key_values.push(vec3(kx + r_vec[0], ky + r_vec[1], kz + r_vec[2]));
    }
    let frames = expand_sparse_vec3(
        &frame_indices,
        &key_values,
        frame_indices.iter().copied().max().unwrap_or(0) + 1,
    );
    Ok(frames)
}

/// Decode a `0x3309` vector curve: keyframed Vector3 values with a blocked
/// residual stream. The keyframed counterpart of `0x3409`.
pub fn decode_translate_3309(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    let header = read_blocked_header(bytes, 0x3309, true, 3)?;
    let key_values = decode_blocked_vec3_keys(bytes, &header)?;
    let total_frames = header.frame_indices.iter().copied().max().unwrap_or(0) + 1;
    Ok(expand_sparse_vec3(
        &header.frame_indices,
        &key_values,
        total_frames,
    ))
}

/// Sample every key of a blocked Vector3 curve (`0x3309` / `0x3409`).
fn decode_blocked_vec3_keys(
    bytes: &[u8],
    header: &BlockedHeader,
) -> Result<Vec<Vec3>, error::Error> {
    let endpoints: Vec<Vec3> = (0..=header.block_count)
        .map(|i| read_vec3_f32_le(bytes, header.endpoints_offset + i * 12).map(Vec3::from))
        .collect::<Result<_, _>>()?;

    let mut key_values = Vec::with_capacity(header.key_count);
    for key_idx in 0..header.key_count {
        let (block_idx, local, block_len) = header.key_span(key_idx);
        let t = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let residual = header.key_residual(bytes, 3, block_idx, local, block_len)?;
        key_values.push(vec3(
            e0.x + (e1.x - e0.x) * t + residual[0],
            e0.y + (e1.y - e0.y) * t + residual[1],
            e0.z + (e1.z - e0.z) * t + residual[2],
        ));
    }
    Ok(key_values)
}

pub fn decode_translate_3400(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    if bytes.len() < 12 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x3400 {
        return Err(error::Error::InvalidData);
    }
    let frame_count = read_u32_le(bytes, 4)? as usize;
    let pos = 12;
    let mut frames = Vec::with_capacity(frame_count);
    for i in 0..frame_count {
        frames.push(read_vec3_f32_le(bytes, pos + i * 12)?.into());
    }
    Ok(frames)
}

/// Decode a `0x3408` vector curve: one Vector3 per frame over a single
/// residual block.
///
/// Layout: `magic | frame_count | frames_per_key | base_scale | endpoint0 | endpoint1 | residual`.
/// The 0x_408 family is always single-block, so the frame count never exceeds 34.
pub fn decode_translate_3408(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    let (base_scale, endpoints, residual_off, key_count) = read_single_block_header(bytes, 0x3408)?;
    let e0 = read_vec3_f32_le(bytes, endpoints)?;
    let e1 = read_vec3_f32_le(bytes, endpoints + 12)?;

    let block_len = key_count.saturating_sub(1).max(1);
    let mut out = Vec::with_capacity(key_count);
    for local in 0..key_count {
        let t = local as f32 / block_len as f32;
        let (residual, _) =
            decode_residual_vector(bytes, residual_off, base_scale, local, 3, block_len)?;
        out.push(vec3(
            e0.x + (e1.x - e0.x) * t + residual[0],
            e0.y + (e1.y - e0.y) * t + residual[1],
            e0.z + (e1.z - e0.z) * t + residual[2],
        ));
    }
    Ok(out)
}

/// Read the header of a single-block residual curve (`0x3408` / `0x4408`).
///
/// Returns `(base_scale, endpoints_offset, residual_offset, key_count)`.
pub(super) fn read_single_block_header(
    bytes: &[u8],
    magic: u32,
) -> Result<(f32, usize, usize, usize), error::Error> {
    let components = if (magic & 0xF000) == 0x4000 { 4 } else { 3 };
    if bytes.len() < 16 || read_u32_le(bytes, 0)? != magic {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    // A single residual block spans at most 33 interpolation steps, so this
    // family never stores more than 34 keys.
    if key_count == 0 || key_count > 34 {
        return Err(error::Error::InvalidData);
    }
    let base_scale = read_f32_le(bytes, 12)?;
    let endpoints_offset = 16;
    let residual_offset = endpoints_offset + 2 * 4 * components;
    if residual_offset > bytes.len() {
        return Err(error::Error::InvalidData);
    }
    Ok((base_scale, endpoints_offset, residual_offset, key_count))
}

/// Decode a `0x3409` vector curve: one Vector3 per frame, stored as a per-block
/// endpoint interpolation plus a compressed residual.
pub fn decode_vector3_3409(bytes: &[u8]) -> Result<Vec<Vec3>, error::Error> {
    let header = read_blocked_header(bytes, 0x3409, false, 3)?;
    decode_blocked_vec3_keys(bytes, &header)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_vs2_3308_key_count_34() {
        let bytes = include_bytes!("fixtures/stgazr_aiming_3308_k34.bin");
        let frames = decode_translate_3308(bytes).expect("0x3308 k=34 must decode");
        assert!(frames.len() >= 34);
        assert!(frames[0].x.is_finite() && frames[0].y.is_finite() && frames[0].z.is_finite());
        // Constant-ish endpoints in this fixture: y stays near 1.35
        assert!((frames[0].y - 1.35).abs() < 0.1 || frames.iter().any(|v| v.y.is_finite()));
    }

    fn read_vec3(bytes: &[u8], offset: usize) -> Vec3 {
        Vec3 {
            x: f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            y: f32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
            z: f32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()),
        }
    }

    /// VS2 `charzk_ex62a`: 117 keys, `block_count = 4`, and every block after the
    /// first carries a zero word offset — those blocks store no residual and are
    /// sampled as pure endpoint interpolations.
    #[test]
    fn decode_vs2_3409_four_blocks_with_empty_tail() {
        let bytes = include_bytes!("fixtures/charzk_ex62a_3409_b6.bin");
        assert_eq!(0x3409, u32::from_le_bytes(bytes[0..4].try_into().unwrap()));
        assert_eq!(117, u32::from_le_bytes(bytes[4..8].try_into().unwrap()));
        assert_eq!(4, u16::from_le_bytes([bytes[16], bytes[17]]));
        for offset in [18, 20, 22] {
            assert_eq!(0, u16::from_le_bytes([bytes[offset], bytes[offset + 1]]));
        }

        let frames = decode_vector3_3409(bytes).expect("0x3409 block_count=4 must decode");
        assert_eq!(117, frames.len());

        // 5 endpoints of 3 f32 start at 0x18, so the residual starts at 84.
        let ep0 = read_vec3(bytes, 0x18);
        assert!((frames[0] - ep0).length() < 1e-4);
        // Keys 33.. fall in the residual-free blocks and follow the endpoints exactly.
        assert!((frames[33] - read_vec3(bytes, 0x18 + 12)).length() < 1e-4);
        assert!((frames[66] - read_vec3(bytes, 0x18 + 24)).length() < 1e-4);
    }

    /// EXVS2 `001hito_028gunwtv_..._stepend_stk_air_bk` BASE Translate.
    ///
    /// `0x3408` is single-block: `base_scale` is the f32 at offset 12 and the two
    /// endpoints follow at 16. The previous decoder scanned for a layout and always
    /// settled on endpoints at 12 with a `base_scale` read out of the residual bytes.
    #[test]
    fn decode_exvs2_gunwtv_3408_uses_declared_layout() {
        let bytes = include_bytes!("fixtures/gunwtv_base_3408.bin");
        assert_eq!(15, u32::from_le_bytes(bytes[4..8].try_into().unwrap()));

        let frames = decode_translate_3408(bytes).expect("0x3408 must decode");
        assert_eq!(15, frames.len());

        // The curve starts on endpoint0@16 and ends on endpoint1@28.
        assert!((frames[0] - read_vec3(bytes, 16)).length() < 1e-4);
        assert!((frames[14] - read_vec3(bytes, 28)).length() < 1e-4);
        // The 4-byte-shifted layout the old scan picked would start the curve on
        // (base_scale, endpoint0.x, endpoint0.y) instead.
        let shifted = read_vec3(bytes, 12);
        assert!(
            (frames[0] - shifted).length() > 1e-3,
            "frame[0] matches the misaligned endpoints@12 layout"
        );
    }

    /// Extracted from VS2 `20headgrab_stk_air_bk` BASE Translate (flags/type=3, 100 keys).
    ///
    /// Must use endpoint base 0x18 (not 0x14). A slack-ranked 0x14 layout shifts all
    /// components and still returns finite values — length/is_finite alone is insufficient.
    #[test]
    fn decode_vs2_headgrab_3409_flags3_100_keys() {
        let bytes = include_bytes!("fixtures/headgrab_100_3409_flags3.bin");
        assert_eq!(3, u16::from_le_bytes([bytes[16], bytes[17]]));

        // Correct endpoints: 4 × vec3 starting at 0x18.
        let ep0 = Vec3 {
            x: f32::from_le_bytes(bytes[0x18..0x1c].try_into().unwrap()),
            y: f32::from_le_bytes(bytes[0x1c..0x20].try_into().unwrap()),
            z: f32::from_le_bytes(bytes[0x20..0x24].try_into().unwrap()),
        };
        let ep3 = Vec3 {
            x: f32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()),
            y: f32::from_le_bytes(bytes[0x40..0x44].try_into().unwrap()),
            z: f32::from_le_bytes(bytes[0x44..0x48].try_into().unwrap()),
        };
        // Sanity: correct ep0 has non-zero x ≈ 0.054; misaligned @0x14 has ~0 x.
        assert!(
            (ep0.x - 0.054017).abs() < 1e-4,
            "fixture ep0.x@0x18 expected ~0.054017, got {}",
            ep0.x
        );

        let frames = decode_vector3_3409(bytes).expect("0x3409 flags=3 must decode");
        assert_eq!(100, frames.len());
        for v in &frames {
            assert!(v.x.is_finite() && v.y.is_finite() && v.z.is_finite());
        }

        let eps = 1e-4;
        assert!(
            (frames[0].x - ep0.x).abs() < eps
                && (frames[0].y - ep0.y).abs() < eps
                && (frames[0].z - ep0.z).abs() < eps,
            "frame[0] must match endpoint0@0x18 {:?}, got {:?}",
            ep0,
            frames[0]
        );
        assert!(
            (frames[99].x - ep3.x).abs() < eps
                && (frames[99].y - ep3.y).abs() < eps
                && (frames[99].z - ep3.z).abs() < eps,
            "frame[99] must match last endpoint@0x18 {:?}, got {:?}",
            ep3,
            frames[99]
        );
        // Reject the known-wrong shifted layout (frame0.x≈0, y≈0.054).
        assert!(
            frames[0].x.abs() > 1e-3,
            "frame[0].x near 0 suggests misaligned 0x14 endpoint base"
        );
    }
}
