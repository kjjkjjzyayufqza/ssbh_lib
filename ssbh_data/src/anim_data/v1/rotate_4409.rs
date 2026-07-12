use glam::Quat;

use super::common::{
    block_index_for_key, compute_block_count, compute_block_len, compute_block_qcounts,
    decode_residual_vector, quat_normalize, read_f32_le, read_u16_le, read_u32_le, read_vec4_f32_le,
};
use crate::anim_data::error;

pub fn decode_rotate_4409(bytes: &[u8]) -> Result<Vec<Quat>, error::Error> {
    if bytes.len() < 0x14 {
        return Err(error::Error::InvalidData);
    }
    if read_u32_le(bytes, 0)? != 0x4409 {
        return Err(error::Error::InvalidData);
    }
    let key_count = read_u32_le(bytes, 4)? as usize;
    let _unk1 = read_f32_le(bytes, 8)?;
    let base_scale = read_f32_le(bytes, 12)?;
    let flags = read_u16_le(bytes, 16)?;
    let _bits = read_u16_le(bytes, 18)?;

    if key_count == 0 {
        return Ok(vec![Quat::IDENTITY]);
    }

    let block_count = compute_block_count(key_count);
    let endpoint_count = block_count + 1;

    let mut best: Option<(Vec<Quat>, usize, usize)> = None;
    // Prefer: lower residual slack, then higher comp_bits, then earlier residual.
    let mut best_key: Option<(usize, i32, usize)> = None;

    // flags/type with low bits odd (3,5,…): extra u32 after 0x14 header → endpoints @0x18.
    // Observed VS2: flags=2 (base 0x14), flags=3/5 (base 0x18 + optional mid gap before residual).
    let endpoint_bases: &[usize] = if (flags & 1) != 0 {
        &[0x18]
    } else {
        &[0x14, 0x18]
    };

    for &endpoint_base in endpoint_bases {
        let endpoints_size = endpoint_count * 16;
        let endpoints_end = endpoint_base + endpoints_size;
        if endpoints_end > bytes.len() {
            continue;
        }
        if (endpoint_base % 4) != 0 {
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
            max_abs = max_abs
                .max(q.x.abs())
                .max(q.y.abs())
                .max(q.z.abs())
                .max(q.w.abs());
            endpoints.push(Quat::from_array(q.to_array()));
        }
        if !ok || !max_abs.is_finite() || max_abs > 1.0e6 {
            continue;
        }

        // Some flags/type variants insert a short mid region between endpoints and residual
        // (not always zero-length). Scan small pads like 0x3409 layout inference.
        for pad in (0..=0x60).step_by(4) {
            let residual_off = endpoints_end + pad;
            if residual_off >= bytes.len() || (residual_off % 4) != 0 {
                continue;
            }

            for comp_bits in [4usize, 3, 2, 1] {
                let end_off = match walk_residual_4409(
                    bytes,
                    residual_off,
                    base_scale,
                    comp_bits,
                    key_count,
                    block_count,
                ) {
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
                // Keep searching pads/comp_bits for a tighter residual fit.
            }
        }
    }

    let (endpoints, residual_off, comp_bits) = best.ok_or(error::Error::InvalidData)?;

    let q_counts = compute_block_qcounts(bytes, residual_off, base_scale, comp_bits, key_count)?;
    let mut prefix_words = vec![0usize];
    for q in &q_counts {
        prefix_words.push(prefix_words.last().copied().unwrap_or(0) + *q);
    }

    let mut out = Vec::with_capacity(key_count);
    for key_idx in 0..key_count {
        let block_idx = block_index_for_key(key_idx, block_count);
        let local = key_idx - 33 * block_idx;
        let mut block_len = compute_block_len(key_count, block_idx);
        if block_len == 0 {
            block_len = 1;
        }
        let t_block = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let k = Quat::from_xyzw(
            e0.x + (e1.x - e0.x) * t_block,
            e0.y + (e1.y - e0.y) * t_block,
            e0.z + (e1.z - e0.z) * t_block,
            e0.w + (e1.w - e0.w) * t_block,
        );
        let rs = residual_off + 4 * prefix_words[block_idx];
        let (r_vec, _) =
            decode_residual_vector(bytes, rs, base_scale, local, comp_bits, block_len)?;
        let q = quat_normalize(
            k.x + r_vec.first().copied().unwrap_or(0.0),
            k.y + r_vec.get(1).copied().unwrap_or(0.0),
            k.z + r_vec.get(2).copied().unwrap_or(0.0),
            k.w + r_vec.get(3).copied().unwrap_or(0.0),
        );
        out.push(q);
    }
    Ok(out)
}

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
        let (_, end_off) =
            decode_residual_vector(payload, cursor, base_scale, 1, comp_bits, block_len)?;
        let delta = end_off.saturating_sub(cursor);
        if delta == 0 || (delta % 4) != 0 {
            return Err(error::Error::InvalidData);
        }
        cursor = end_off;
    }
    Ok(cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_vs2_orphn2_4409_flags5_100_keys() {
        let bytes = include_bytes!("fixtures/orphn2_ex51a_4409_flags5.bin");
        let frames = decode_rotate_4409(bytes).expect("flags=5");
        assert_eq!(158, frames.len());
        let ep0 = Quat::from_xyzw(
            f32::from_le_bytes(bytes[0x18..0x1c].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x1c..0x20].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x20..0x24].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x24..0x28].try_into().unwrap()),
        )
        .normalize();
        let eps = 1e-3;
        assert!((frames[0].x - ep0.x).abs() < eps);
        assert!((frames[0].y - ep0.y).abs() < eps);
        assert!((frames[0].z - ep0.z).abs() < eps);
        assert!((frames[0].w - ep0.w).abs() < eps);
    }

    /// Extracted from VS2 `20headgrab_stk_air_bk` MUNE1 Rotate (flags/type=3, 100 keys).
    /// Endpoints must start at 0x18 when flags==3.
    #[test]
    fn decode_vs2_headgrab_4409_flags3_100_keys() {
        let bytes = include_bytes!("fixtures/headgrab_100_4409_flags3.bin");
        assert_eq!(3, u16::from_le_bytes([bytes[16], bytes[17]]));

        let ep0 = Quat::from_xyzw(
            f32::from_le_bytes(bytes[0x18..0x1c].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x1c..0x20].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x20..0x24].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x24..0x28].try_into().unwrap()),
        );
        let ep3 = Quat::from_xyzw(
            f32::from_le_bytes(bytes[0x48..0x4c].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x4c..0x50].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x50..0x54].try_into().unwrap()),
            f32::from_le_bytes(bytes[0x54..0x58].try_into().unwrap()),
        );

        let frames = decode_rotate_4409(bytes).expect("0x4409 flags=3 must decode");
        assert_eq!(100, frames.len());
        for q in &frames {
            assert!(q.x.is_finite() && q.y.is_finite() && q.z.is_finite() && q.w.is_finite());
            let n2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
            assert!((n2 - 1.0).abs() < 1e-2, "expected near-unit quat, n2={n2}");
        }

        let eps = 1e-4;
        // Endpoints may need normalize; compare after normalize.
        let ep0n = ep0.normalize();
        let ep3n = ep3.normalize();
        assert!(
            (frames[0].x - ep0n.x).abs() < eps
                && (frames[0].y - ep0n.y).abs() < eps
                && (frames[0].z - ep0n.z).abs() < eps
                && (frames[0].w - ep0n.w).abs() < eps,
            "frame[0] must match ep0@0x18 {:?}, got {:?}",
            ep0n,
            frames[0]
        );
        assert!(
            (frames[99].x - ep3n.x).abs() < eps
                && (frames[99].y - ep3n.y).abs() < eps
                && (frames[99].z - ep3n.z).abs() < eps
                && (frames[99].w - ep3n.w).abs() < eps,
            "frame[99] must match last ep@0x18 {:?}, got {:?}",
            ep3n,
            frames[99]
        );
    }
}
