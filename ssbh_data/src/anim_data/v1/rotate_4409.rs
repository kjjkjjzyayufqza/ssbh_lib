use glam::Quat;

use super::common::{quat_normalize, read_blocked_header, read_vec4_f32_le};
use crate::anim_data::error;

/// Decode a `0x4409` rotation curve: one quaternion per frame, stored as a
/// per-block endpoint interpolation plus a compressed residual.
pub fn decode_rotate_4409(bytes: &[u8]) -> Result<Vec<Quat>, error::Error> {
    let header = read_blocked_header(bytes, 0x4409, false, 4)?;

    let endpoints: Vec<Quat> = (0..=header.block_count)
        .map(|i| {
            read_vec4_f32_le(bytes, header.endpoints_offset + i * 16)
                .map(|v| Quat::from_array(v.to_array()))
        })
        .collect::<Result<_, _>>()?;

    let mut out = Vec::with_capacity(header.key_count);
    for key_idx in 0..header.key_count {
        let (block_idx, local, block_len) = header.key_span(key_idx);
        let t = local as f32 / block_len as f32;
        let e0 = endpoints[block_idx];
        let e1 = endpoints[block_idx + 1];
        let residual = header.key_residual(bytes, 4, block_idx, local, block_len)?;
        out.push(quat_normalize(
            e0.x + (e1.x - e0.x) * t + residual[0],
            e0.y + (e1.y - e0.y) * t + residual[1],
            e0.z + (e1.z - e0.z) * t + residual[2],
            e0.w + (e1.w - e0.w) * t + residual[3],
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(bytes: &[u8], offset: usize) -> Quat {
        Quat::from_xyzw(
            f32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            f32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()),
            f32::from_le_bytes(bytes[offset + 8..offset + 12].try_into().unwrap()),
            f32::from_le_bytes(bytes[offset + 12..offset + 16].try_into().unwrap()),
        )
        .normalize()
    }

    fn assert_close(actual: Quat, expected: Quat, label: &str) {
        let eps = 1e-4;
        assert!(
            (actual.x - expected.x).abs() < eps
                && (actual.y - expected.y).abs() < eps
                && (actual.z - expected.z).abs() < eps
                && (actual.w - expected.w).abs() < eps,
            "{label}: expected {expected:?}, got {actual:?}"
        );
    }

    /// Extracted from VS2 `20headgrab_*`: 100 keys, `block_count = 3`, so the
    /// endpoint table starts at 0x18.
    #[test]
    fn decode_vs2_headgrab_4409_three_blocks_100_keys() {
        let bytes = include_bytes!("fixtures/headgrab_100_4409_flags3.bin");
        assert_eq!(3, u16::from_le_bytes([bytes[16], bytes[17]]));

        let frames = decode_rotate_4409(bytes).expect("0x4409 block_count=3 must decode");
        assert_eq!(100, frames.len());
        for q in &frames {
            let n2 = q.x * q.x + q.y * q.y + q.z * q.z + q.w * q.w;
            assert!((n2 - 1.0).abs() < 1e-2, "expected near-unit quat, n2={n2}");
        }
        assert_close(frames[0], endpoint(bytes, 0x18), "frame[0]");
        assert_close(frames[99], endpoint(bytes, 0x48), "frame[99]");
    }

    /// Extracted from VS2 `orphn2_ex51a`: 158 keys, `block_count = 5`, so the
    /// endpoint table starts at 0x1C. The trailing two blocks carry a zero word
    /// offset, meaning they store no residual and are pure endpoint lerps.
    #[test]
    fn decode_vs2_orphn2_4409_five_blocks_with_empty_tail() {
        let bytes = include_bytes!("fixtures/orphn2_ex51a_4409_flags5.bin");
        assert_eq!(5, u16::from_le_bytes([bytes[16], bytes[17]]));
        assert_eq!(0, u16::from_le_bytes([bytes[22], bytes[23]]));
        assert_eq!(0, u16::from_le_bytes([bytes[24], bytes[25]]));

        let frames = decode_rotate_4409(bytes).expect("empty trailing blocks must decode");
        assert_eq!(158, frames.len());
        assert_close(frames[0], endpoint(bytes, 0x1C), "frame[0]");

        // Blocks are continuous across their shared endpoints.
        for boundary in [33usize, 66, 99, 132] {
            let step = frames[boundary]
                .dot(frames[boundary - 1])
                .abs()
                .min(1.0)
                .acos()
                * 2.0;
            assert!(
                step.to_degrees() < 5.0,
                "discontinuity at block boundary {boundary}: {}deg",
                step.to_degrees()
            );
        }
    }

    /// A `block_count` that disagrees with the key count means the buffer is not
    /// this layout. Decoding must fail instead of guessing an endpoint offset.
    #[test]
    fn decode_rotate_4409_rejects_inconsistent_block_count() {
        let mut bytes = include_bytes!("fixtures/headgrab_100_4409_flags3.bin").to_vec();
        bytes[16] = 2;
        assert!(decode_rotate_4409(&bytes).is_err());
    }
}
