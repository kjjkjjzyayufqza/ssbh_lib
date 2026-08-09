use glam::Quat;

use super::common::{expand_sparse_quat, quat_normalize, read_blocked_header, read_vec4_f32_le};
use crate::anim_data::error;

/// Decode a `0x4309` rotation curve: keyframed quaternions with a blocked
/// residual stream.
///
/// Identical to `0x4409` except every key carries a `u8` frame index, so the
/// decoded keys are interpolated out to the frame timeline afterwards.
pub fn decode_rotate_4309(bytes: &[u8]) -> Result<Vec<Quat>, error::Error> {
    let header = read_blocked_header(bytes, 0x4309, true, 4)?;

    let endpoints: Vec<Quat> = (0..=header.block_count)
        .map(|i| {
            read_vec4_f32_le(bytes, header.endpoints_offset + i * 16)
                .map(|v| Quat::from_array(v.to_array()))
        })
        .collect::<Result<_, _>>()?;

    let mut key_quats = Vec::with_capacity(header.key_count);
    for key_idx in 0..header.key_count {
        let (block_idx, local, block_len) = header.key_span(key_idx);
        let t = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let residual = header.key_residual(bytes, 4, block_idx, local, block_len)?;
        key_quats.push(quat_normalize(
            e0.x + (e1.x - e0.x) * t + residual[0],
            e0.y + (e1.y - e0.y) * t + residual[1],
            e0.z + (e1.z - e0.z) * t + residual[2],
            e0.w + (e1.w - e0.w) * t + residual[3],
        ));
    }

    let keys: Vec<(f32, f32, f32, f32)> = key_quats.iter().map(|q| (q.x, q.y, q.z, q.w)).collect();
    let total_frames = header.frame_indices.iter().copied().max().unwrap_or(0) + 1;
    Ok(expand_sparse_quat(
        &header.frame_indices,
        &keys,
        total_frames,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EXVS2 `001hito_028gunwtv_..._stepbgn_stk_air_fr` SAKOTSU_L: 35 keys over
    /// 45 frames, `block_count = 2`, so the endpoint table starts 8 bytes after
    /// `base_scale`. The previous decoder hardcoded the `block_count = 3` offset
    /// and inferred the rest, which shifted every endpoint by one f32 and turned
    /// this clavicle into a 124 deg/frame flicker.
    #[test]
    fn decode_exvs2_gunwtv_4309_two_blocks() {
        let bytes = include_bytes!("fixtures/gunwtv_sakotsu_l_4309_2blocks.bin");
        // key_count 35 → frame indices end at 47, padded to 48.
        assert_eq!(35, u32::from_le_bytes(bytes[4..8].try_into().unwrap()));
        assert_eq!(2, u16::from_le_bytes([bytes[52], bytes[53]]));

        let frames = decode_rotate_4309(bytes).expect("0x4309 block_count=2 must decode");
        assert_eq!(45, frames.len());

        // Endpoint 0 lives at base_scale + 8 and is already a unit quaternion.
        let endpoint0 = Quat::from_xyzw(
            f32::from_le_bytes(bytes[56..60].try_into().unwrap()),
            f32::from_le_bytes(bytes[60..64].try_into().unwrap()),
            f32::from_le_bytes(bytes[64..68].try_into().unwrap()),
            f32::from_le_bytes(bytes[68..72].try_into().unwrap()),
        );
        assert!((endpoint0.length() - 1.0).abs() < 1e-4);
        assert!(
            (frames[0].dot(endpoint0).abs() - 1.0).abs() < 1e-4,
            "frame[0] {:?} must match endpoint0 {endpoint0:?}",
            frames[0]
        );

        // A clavicle is one of the least mobile bones; consecutive frames must
        // stay close instead of flipping.
        let worst = frames
            .windows(2)
            .map(|w| 2.0 * w[0].dot(w[1]).abs().min(1.0).acos().to_degrees())
            .fold(0.0f32, f32::max);
        assert!(worst < 5.0, "max per-frame rotation step {worst} deg");
    }

    #[test]
    fn decode_rotate_4309_rejects_inconsistent_block_count() {
        let mut bytes = include_bytes!("fixtures/gunwtv_sakotsu_l_4309_2blocks.bin").to_vec();
        bytes[52] = 3;
        assert!(decode_rotate_4309(&bytes).is_err());
    }
}
