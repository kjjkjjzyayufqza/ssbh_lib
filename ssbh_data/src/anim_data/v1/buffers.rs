use std::io::Cursor;

use binrw::{
    BinRead, BinReaderExt,
    helpers::{count_with, until_eof},
};
use glam::{Quat, Vec3};
use ssbh_lib::{Vector3, Vector4, formats::anim::TrackTypeV1};

use crate::{
    anim_data::{
        TrackValues, Transform, TransformFlags, UvTransform,
        error::Error,
        v1::{rotate_4309::*, rotate_4409::*, rotate_basic::*, translate::*},
    },
    read_vec3,
};

// TODO: Organize this in compression.rs similar to version 2.0+
// TODO: Is the magic multiple fields for const, data type, etc?
#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub enum V12BufferData {
    // scalar
    #[br(magic(0x1003u32))]
    Unk1003(u16),

    #[br(magic(0x1013u32))]
    Unk1013(u16),

    // vector2
    #[br(magic(0x2003u32))]
    Unk2003((f32, f32)),

    // vector3
    #[br(magic(0x3003u32))]
    Unk3003(Vector3),

    Unk3300(Unk3300),

    #[br(magic(0x3308u32))]
    Unk3308(Unk3308),

    #[br(magic(0x3309u32))]
    Unk3309(Unk3309),

    #[br(magic(0x3408u32))]
    Unk3408(Unk3408),

    #[br(magic(0x3409u32))]
    Unk3409(Unk3409),

    // vector4
    #[br(magic(0x4003u32))]
    Unk4003(Vector4),

    #[br(magic(0x4300u32))]
    Unk4300(Unk4300),

    #[br(magic(0x4308u32))]
    Unk4308(Unk4308),

    #[br(magic(0x4309u32))]
    Unk4309(Unk4309),

    #[br(magic(0x4408u32))]
    Unk4408(Unk4408),

    #[br(magic(0x4409u32))]
    Unk4409(Unk4409),
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
#[br(magic(0x3300u32))]
pub struct Unk3300 {
    pub frame_count: u32,
    pub unk1: f32,

    #[br(count = frame_count, align_after = 4)] // align to float boundary
    pub frame_indices: Vec<u8>,

    #[br(parse_with = count_with(frame_count as usize, read_vec3))]
    pub values: Vec<Vec3>,
}

// Single-block residual curves (`0x_308` / `0x_408`): two endpoints, one
// residual stream, at most 34 keys. The `0x_308` variants add a `u8` frame
// index per key. Decoded by `translate::read_single_block_header`.

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk3308 {
    pub key_count: u32,
    pub unk1: f32,

    #[br(count = key_count, align_after = 4)]
    pub frame_indices: Vec<u8>,

    pub base_scale: f32,
    pub endpoints: [Vector3; 2],

    #[br(parse_with = until_eof)]
    pub residual: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk4308 {
    pub key_count: u32,
    pub unk1: f32,

    #[br(count = key_count, align_after = 4)]
    pub frame_indices: Vec<u8>,

    pub base_scale: f32,
    pub endpoints: [Vector4; 2],

    #[br(parse_with = until_eof)]
    pub residual: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk3408 {
    pub frame_count: u32,
    pub unk1: f32,
    pub base_scale: f32,
    pub endpoints: [Vector3; 2],

    #[br(parse_with = until_eof)]
    pub residual: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk4408 {
    pub frame_count: u32,
    pub unk1: f32,
    pub base_scale: f32,
    pub endpoints: [Vector4; 2],

    #[br(parse_with = until_eof)]
    pub residual: Vec<u8>,
}

// Blocked residual curves (`0x_309` / `0x_409`): the clip is split into 33-key
// blocks, `block_count == ceil((key_count - 1) / 33)`. `block_words` gives the
// u32-word offset of every block after the first, relative to the residual
// stream, with 0 marking a block that stores no residual. Decoded by
// `common::read_blocked_header`.

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk3309 {
    pub key_count: u32,
    pub unk1: f32,

    #[br(count = key_count, align_after = 4)]
    pub frame_indices: Vec<u8>,

    pub base_scale: f32,
    pub block_count: u16,

    #[br(count = block_count.saturating_sub(1), align_after = 4)]
    pub block_words: Vec<u16>,

    #[br(count = block_count as usize + 1)]
    pub endpoints: Vec<Vector3>,

    #[br(parse_with = until_eof)]
    pub residual: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk4309 {
    pub key_count: u32,
    pub unk1: f32,

    #[br(count = key_count, align_after = 4)]
    pub frame_indices: Vec<u8>,

    pub base_scale: f32,
    pub block_count: u16,

    #[br(count = block_count.saturating_sub(1), align_after = 4)]
    pub block_words: Vec<u16>,

    #[br(count = block_count as usize + 1)]
    pub endpoints: Vec<Vector4>,

    #[br(parse_with = until_eof)]
    pub residual: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk3409 {
    pub key_count: u32,
    pub unk1: f32,
    pub base_scale: f32,
    pub block_count: u16,

    #[br(count = block_count.saturating_sub(1), align_after = 4)]
    pub block_words: Vec<u16>,

    #[br(count = block_count as usize + 1)]
    pub endpoints: Vec<Vector3>,

    #[br(parse_with = until_eof)]
    pub residual: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk4409 {
    pub key_count: u32,
    pub unk1: f32,
    pub base_scale: f32,
    pub block_count: u16,

    #[br(count = block_count.saturating_sub(1), align_after = 4)]
    pub block_words: Vec<u16>,

    #[br(count = block_count as usize + 1)]
    pub endpoints: Vec<Vector4>,

    #[br(parse_with = until_eof)]
    pub residual: Vec<u8>,
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
pub struct Unk4300 {
    pub frame_count: u32,
    pub unk1: f32,

    #[br(count = frame_count, align_after = 4)]
    pub frame_indices: Vec<u8>,

    #[br(count = frame_count)]
    pub values: Vec<Vector4>,
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
enum V12Values {
    Uint(u16),
    /// Multi-frame boolean / u16 stream (e.g. Visibility 0x1019).
    Uints(Vec<u16>),
    Float(f32),
    Vec2((f32, f32)),
    Vec3(Vec<Vec3>),
    Quat(Vec<Quat>),
    /// Constant (0x5014) or multi-frame (0x5019) UV transform stream.
    Uv(Vec<UvTransform>),
}

/// Expand or shrink sample lists to match the VS2/EXVS2 animation frame count.
fn reconcile_samples<T: Clone>(samples: Vec<T>, frame_count: usize, default: T) -> Vec<T> {
    let frame_count = frame_count.max(1);
    if samples.is_empty() {
        return vec![default; frame_count];
    }
    if samples.len() == 1 {
        return vec![samples[0].clone(); frame_count];
    }
    if samples.len() < frame_count {
        let last = samples.last().cloned().unwrap_or(default);
        let mut out = samples;
        out.resize(frame_count, last);
        return out;
    }
    if samples.len() > frame_count {
        let mut out = samples;
        out.truncate(frame_count);
        return out;
    }
    samples
}

struct PropertyData {
    scales: Vec<Vec3>,
    rotations: Vec<Quat>,
    translations: Vec<Vec3>,
    visibilities: Vec<bool>,
    uv_transforms: Vec<UvTransform>,
}
pub fn read_track_values_v12(
    track: &ssbh_lib::formats::anim::TrackV1,
    buffers: &[ssbh_lib::SsbhByteBuffer],
    animation_frame_count: usize,
) -> Result<(TrackValues, bool, TransformFlags), Error> {
    let mut compensate_scale = false;
    // Collect parsed property data
    let mut property_data = PropertyData {
        scales: Vec::new(),
        rotations: Vec::new(),
        translations: Vec::new(),
        visibilities: Vec::new(),
        uv_transforms: Vec::new(),
    };
    let mut has_scale = false;
    let mut has_rotate = false;
    let mut has_translate = false;
    for property in &track.properties.elements {
        let property_name = property.name.to_string_lossy();
        match property_name.as_str() {
            "Scale" => has_scale = true,
            "Rotate" => has_rotate = true,
            "Translate" => has_translate = true,
            _ => {}
        }
        let data =
            buffers
                .get(property.buffer_index as usize)
                .ok_or(Error::BufferIndexOutOfRange {
                    buffer_index: property.buffer_index as usize,
                    buffer_count: buffers.len(),
                })?;

        add_property(
            &mut compensate_scale,
            &mut property_data,
            property_name,
            data,
        )?;
    }

    // Anim v1.2 is property-sparse: many bones only encode Rotate.
    // Missing channels must *not* be treated as authored zeros/identity when applied
    // as absolute local TRS — that collapses the skeleton. Match SSBH TransformFlags:
    // override_* = true means "use skeleton rest for this channel".
    let transform_flags = TransformFlags {
        override_translation: !has_translate,
        override_rotation: !has_rotate,
        override_scale: !has_scale,
        override_compensate_scale: false,
    };

    let values = match track.track_type {
        TrackTypeV1::Transform => {
            let scales = reconcile_samples(property_data.scales, animation_frame_count, Vec3::ONE);
            let rotations = reconcile_samples(
                property_data.rotations,
                animation_frame_count,
                Quat::IDENTITY,
            );
            let translations = reconcile_samples(
                property_data.translations,
                animation_frame_count,
                Vec3::ZERO,
            );

            let mut transforms = Vec::with_capacity(animation_frame_count.max(1));
            for frame_idx in 0..animation_frame_count.max(1) {
                transforms.push(Transform {
                    scale: scales[frame_idx].to_array().into(),
                    rotation: Quat::from_array(rotations[frame_idx].to_array()),
                    translation: translations[frame_idx].to_array().into(),
                });
            }

            TrackValues::Transform(transforms)
        }
        TrackTypeV1::Visibility => TrackValues::Boolean(reconcile_samples(
            property_data.visibilities,
            animation_frame_count,
            true,
        )),
        TrackTypeV1::UvTransform => {
            let default_uv = UvTransform {
                scale_u: 1.0,
                scale_v: 1.0,
                rotation: 0.0,
                translate_u: 0.0,
                translate_v: 0.0,
            };
            TrackValues::UvTransform(reconcile_samples(
                property_data.uv_transforms,
                animation_frame_count,
                default_uv,
            ))
        }
    };
    Ok((values, compensate_scale, transform_flags))
}

fn add_property(
    compensate_scale: &mut bool,
    property_data: &mut PropertyData,
    property_name: String,
    data: &ssbh_lib::SsbhByteBuffer,
) -> Result<(), Error> {
    let value = read_property_value_v12(&data.elements, &property_name)?;
    match property_name.as_str() {
        "CompensateScale" => match value {
            V12Values::Uint(v) => {
                *compensate_scale = v != 0;
            }
            V12Values::Float(f) => {
                *compensate_scale = f != 0.0;
            }
            _ => {
                return Err(Error::UnexpectedV12PropertyValue { property_name });
            }
        },
        "Scale" => match value {
            V12Values::Vec3(values) => {
                property_data.scales.extend(values);
            }
            _ => {
                return Err(Error::UnexpectedV12PropertyValue { property_name });
            }
        },
        "Rotate" => match value {
            V12Values::Vec3(values) => {
                // Euler angles (degrees/radians as stored) → quaternion.
                property_data
                    .rotations
                    .extend(values.into_iter().map(|v| euler_to_quaternion(v.into())));
            }
            V12Values::Quat(values) => {
                property_data.rotations.extend(values);
            }
            _ => {
                return Err(Error::UnexpectedV12PropertyValue { property_name });
            }
        },
        "Translate" => match value {
            V12Values::Vec3(values) => {
                property_data.translations.extend(values);
            }
            _ => {
                return Err(Error::UnexpectedV12PropertyValue { property_name });
            }
        },
        "Visibility" => match value {
            V12Values::Uint(v) => {
                property_data.visibilities.push(v != 0);
            }
            V12Values::Uints(values) => {
                property_data
                    .visibilities
                    .extend(values.into_iter().map(|v| v != 0));
            }
            _ => {
                return Err(Error::UnexpectedV12PropertyValue { property_name });
            }
        },
        "UvTransform" => match value {
            V12Values::Uv(values) => {
                property_data.uv_transforms.extend(values);
            }
            _ => {
                return Err(Error::UnexpectedV12PropertyValue { property_name });
            }
        },
        // Unknown property names are ignored for forward compatibility with VS2 variants.
        _ => {}
    }
    Ok(())
}

fn map_v12_decode_err(err: Error, header: u32, property_name: &str) -> Error {
    match err {
        Error::InvalidData => Error::V12PropertyDecodeFailed {
            header,
            property_name: property_name.to_string(),
        },
        other => other,
    }
}

fn read_property_value_v12(bytes: &[u8], property_name: &str) -> Result<V12Values, Error> {
    let mut reader = Cursor::new(bytes);
    let header: u32 = reader.read_le()?;

    let value = match header {
        0x1013 => {
            let value: u16 = reader.read_le()?;
            V12Values::Uint(value)
        }
        // Multi-frame visibility / boolean stream:
        // u32 header (0x1019) + u32 frame_count + frame_count * u16 values.
        0x1019 => {
            let frame_count: u32 = reader.read_le()?;
            let mut values = Vec::with_capacity(frame_count as usize);
            for _ in 0..frame_count {
                let value: u16 = reader.read_le()?;
                values.push(value);
            }
            V12Values::Uints(values)
        }
        0x1003 => {
            let value: f32 = reader.read_le()?;
            V12Values::Float(value)
        }
        0x3003 => {
            // Single scale value
            let scale = reader.read_le::<Vector3>()?;
            V12Values::Vec3(vec![scale.into()])
        }
        0x3200 => V12Values::Vec3(
            decode_translate_3200(bytes)
                .map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x3208 => V12Values::Vec3(
            decode_translate_3208(bytes)
                .map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x3209 => V12Values::Vec3(
            decode_translate_3209(bytes)
                .map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x3300 => V12Values::Vec3(
            decode_translate_3300(bytes)
                .map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x3308 => V12Values::Vec3(
            decode_translate_3308(bytes)
                .map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x3309 => V12Values::Vec3(
            decode_translate_3309(bytes)
                .map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x3400 => V12Values::Vec3(
            decode_translate_3400(bytes)
                .map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x3408 => V12Values::Vec3(
            decode_translate_3408(bytes)
                .map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x3409 => V12Values::Vec3(
            decode_vector3_3409(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4003 => {
            // Single Vector4 (quaternion rotation) - uncompressed
            let rotation = reader.read_le::<[f32; 4]>()?;
            V12Values::Quat(vec![Quat::from_array(rotation)])
        }
        0x4200 => V12Values::Quat(
            decode_rotate_4200(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4208 => V12Values::Quat(
            decode_rotate_4208(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4209 => V12Values::Quat(
            decode_rotate_4209(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4300 => V12Values::Quat(
            decode_rotate_4300(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4308 => V12Values::Quat(
            decode_rotate_4308(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4309 => V12Values::Quat(
            decode_rotate_4309(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4400 => V12Values::Quat(
            decode_rotate_4400(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4408 => V12Values::Quat(
            decode_rotate_4408(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        0x4409 => V12Values::Quat(
            decode_rotate_4409(bytes).map_err(|e| map_v12_decode_err(e, header, property_name))?,
        ),
        // Constant UV transform (VS2/EXVS2 material tracks).
        0x5014 => {
            let scale_u: f32 = reader.read_le()?;
            let scale_v: f32 = reader.read_le()?;
            let rotation: f32 = reader.read_le()?;
            let translate_u: f32 = reader.read_le()?;
            let translate_v: f32 = reader.read_le()?;
            V12Values::Uv(vec![UvTransform {
                scale_u,
                scale_v,
                rotation,
                translate_u,
                translate_v,
            }])
        }
        // Multi-frame UV stream: header + frame_count + N * 5 f32.
        0x5019 => {
            let frame_count: u32 = reader.read_le()?;
            let mut values = Vec::with_capacity(frame_count as usize);
            for _ in 0..frame_count {
                values.push(UvTransform {
                    scale_u: reader.read_le()?,
                    scale_v: reader.read_le()?,
                    rotation: reader.read_le()?,
                    translate_u: reader.read_le()?,
                    translate_v: reader.read_le()?,
                });
            }
            V12Values::Uv(values)
        }
        _ => {
            return Err(Error::UnsupportedV12PropertyHeader {
                header,
                property_name: property_name.to_string(),
            });
        }
    };
    Ok(value)
}

// TODO: use glam for this.
fn euler_to_quaternion(euler: Vector3) -> Quat {
    let (sx, cx) = (euler.x * 0.5).sin_cos();
    let (sy, cy) = (euler.y * 0.5).sin_cos();
    let (sz, cz) = (euler.z * 0.5).sin_cos();

    Quat::from_xyzw(
        sx * cy * cz - cx * sy * sz,
        cx * sy * cz + sx * cy * sz,
        cx * cy * sz - sx * sy * cz,
        cx * cy * cz + sx * sy * sz,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use glam::{quat, vec3};
    use hexlit::hex;
    use pretty_assertions::assert_eq;

    /// Compare decoded property values with a tolerance.
    ///
    /// The residual codecs reconstruct each sample from a quantized DCT stream,
    /// so the last f32 bit depends on the accumulation order. Exact equality
    /// would make these goldens brittle without catching anything real.
    fn assert_values_close(expected: V12Values, actual: V12Values) {
        const EPS: f32 = 1e-5;
        match (&expected, &actual) {
            (V12Values::Vec3(a), V12Values::Vec3(b)) => {
                assert_eq!(a.len(), b.len(), "frame count");
                for (i, (e, g)) in a.iter().zip(b.iter()).enumerate() {
                    assert!(
                        (*e - *g).length() < EPS,
                        "frame {i}: expected {e:?}, got {g:?}"
                    );
                }
            }
            (V12Values::Quat(a), V12Values::Quat(b)) => {
                assert_eq!(a.len(), b.len(), "frame count");
                for (i, (e, g)) in a.iter().zip(b.iter()).enumerate() {
                    assert!(
                        (e.dot(*g).abs() - 1.0).abs() < EPS,
                        "frame {i}: expected {e:?}, got {g:?}"
                    );
                }
            }
            _ => panic!("value kind mismatch: expected {expected:?}, got {actual:?}"),
        }
    }

    // TODO: tests for entire 1.2 track with properties
    // TODO: One test for each anim 1.2 buffer variant.
    #[test]
    fn read_anim_v12_0330() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_15winloop01_sht_gnd_fr.nuanmb, GBL_RT, Scale
        let data = hex!(03300000 0000803f 0000803f 0000803f);
        assert_eq!(
            V12Values::Vec3(vec![vec3(1.0, 1.0, 1.0)]),
            read_property_value_v12(&data, "test").unwrap()
        );
    }

    #[test]
    fn read_anim_v12_0033() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_javelinloop_sht_air_fr.nuanmb, BASE, Translate
        let data = hex!(
            00330000
            02000000
            0000803f 000f0000 46942abf 90d83e3e 00000000 46942abf 4dd83e3e 00000000
        );
        assert_eq!(
            V12Values::Vec3(vec![
                vec3(-0.666325, 0.186373, 0.0),
                vec3(-0.666325, 0.18637294, 0.0),
                vec3(-0.666325, 0.18637286, 0.0),
                vec3(-0.666325, 0.1863728, 0.0),
                vec3(-0.666325, 0.18637273, 0.0),
                vec3(-0.666325, 0.18637267, 0.0),
                vec3(-0.666325, 0.1863726, 0.0),
                vec3(-0.666325, 0.18637253, 0.0),
                vec3(-0.666325, 0.18637246, 0.0),
                vec3(-0.666325, 0.1863724, 0.0),
                vec3(-0.666325, 0.18637232, 0.0),
                vec3(-0.666325, 0.18637227, 0.0),
                vec3(-0.666325, 0.18637219, 0.0),
                vec3(-0.666325, 0.18637213, 0.0),
                vec3(-0.666325, 0.18637206, 0.0),
                vec3(-0.666325, 0.186372, 0.0),
            ]),
            read_property_value_v12(&data, "test").unwrap()
        );
    }

    #[test]
    fn read_anim_v12_0833() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_15winloop01_sht_gnd_fr.nuanmb, BASE, Translate
        let data = hex!(
            08330000
            0c000000
            0000803f
            0004080c
            1118252b
            3034383b
            baf6a63c
            7bbf313e
            3220bb3f
            4a97bebe
            7bbf313e
            6cecba3f
            4a97bebe
            01000100
            01010211
            00000000
            ffffd615
            ff012011
            137ffcbf
            0x4188ea5c
            01000100
            01010211
            00000000
        );
        assert_eq!(
            V12Values::Vec3(vec![
                vec3(0.173582, 1.46192, -0.372248),
                vec3(0.173582, 1.4631298, -0.372248),
                vec3(0.173582, 1.4643394, -0.372248),
                vec3(0.173582, 1.465549, -0.372248),
                vec3(0.173582, 1.4667587, -0.372248),
                vec3(0.173582, 1.4673619, -0.372248),
                vec3(0.173582, 1.4679651, -0.372248),
                vec3(0.173582, 1.4685682, -0.372248),
                vec3(0.173582, 1.4691714, -0.372248),
                vec3(0.173582, 1.4693661, -0.372248),
                vec3(0.173582, 1.4695609, -0.372248),
                vec3(0.173582, 1.4697555, -0.372248),
                vec3(0.173582, 1.4699502, -0.372248),
                vec3(0.173582, 1.4696864, -0.372248),
                vec3(0.173582, 1.4694226, -0.372248),
                vec3(0.173582, 1.4691588, -0.372248),
                vec3(0.173582, 1.468895, -0.372248),
                vec3(0.173582, 1.4686311, -0.372248),
                vec3(0.173582, 1.4680263, -0.372248),
                vec3(0.173582, 1.4674214, -0.372248),
                vec3(0.173582, 1.4668165, -0.372248),
                vec3(0.173582, 1.4662117, -0.372248),
                vec3(0.173582, 1.4656068, -0.372248),
                vec3(0.173582, 1.465002, -0.372248),
                vec3(0.173582, 1.4643971, -0.372248),
                vec3(0.173582, 1.4636115, -0.372248),
                vec3(0.173582, 1.4628259, -0.372248),
                vec3(0.173582, 1.4620403, -0.372248),
                vec3(0.173582, 1.4612546, -0.372248),
                vec3(0.173582, 1.460469, -0.372248),
                vec3(0.173582, 1.4596834, -0.372248),
                vec3(0.173582, 1.4588978, -0.372248),
                vec3(0.173582, 1.4581122, -0.372248),
                vec3(0.173582, 1.4573267, -0.372248),
                vec3(0.173582, 1.456541, -0.372248),
                vec3(0.173582, 1.4557554, -0.372248),
                vec3(0.173582, 1.4549698, -0.372248),
                vec3(0.173582, 1.4541842, -0.372248),
                vec3(0.173582, 1.4536883, -0.372248),
                vec3(0.173582, 1.4531924, -0.372248),
                vec3(0.173582, 1.4526963, -0.372248),
                vec3(0.173582, 1.4522004, -0.372248),
                vec3(0.173582, 1.4517045, -0.372248),
                vec3(0.173582, 1.4512086, -0.372248),
                vec3(0.173582, 1.4511329, -0.372248),
                vec3(0.173582, 1.4510573, -0.372248),
                vec3(0.173582, 1.4509816, -0.372248),
                vec3(0.173582, 1.450906, -0.372248),
                vec3(0.173582, 1.4508303, -0.372248),
                vec3(0.173582, 1.4512398, -0.372248),
                vec3(0.173582, 1.4516493, -0.372248),
                vec3(0.173582, 1.4520588, -0.372248),
                vec3(0.173582, 1.4524683, -0.372248),
                vec3(0.173582, 1.4533961, -0.372248),
                vec3(0.173582, 1.4543238, -0.372248),
                vec3(0.173582, 1.4552516, -0.372248),
                vec3(0.173582, 1.4561794, -0.372248),
                vec3(0.173582, 1.4575663, -0.372248),
                vec3(0.173582, 1.4589531, -0.372248),
                vec3(0.173582, 1.46034, -0.372248),
            ]),
            read_property_value_v12(&data, "test").unwrap()
        );
    }

    #[test]
    fn read_anim_v12_0933() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_guardloop_stk_air_fr.nuanmb, BASE, Translate
        let data = hex!(
            09330000
            2a000000
            0000803f
            00010203040507090b0e1417191b1c1d1e1f2021222324252627282a2c2e31373a3c3e3f4041424344450000
            f00f063f
            0200
            0e00
            00000000 45f0bf3f 00000000
            00000000 eacabb3f 00000000
            00000000 0000c03f 00000000
            01000100 01000812 ffffef1d46080012837f030e81cfd50d230e09ebeec2390150ec08c8effd1420fdffde07f226f720
            01000100 01000812
            01000100 01000211
            150a580a ff002011 f17c738c
            01000100 01000211
        );
        assert_eq!(
            V12Values::Vec3(vec![
                vec3(0.0, 1.49952, 0.0),
                vec3(0.0, 1.4976515, 0.0),
                vec3(0.0, 1.4953662, 0.0),
                vec3(0.0, 1.4924352, 0.0),
                vec3(0.0, 1.4888555, 0.0),
                vec3(0.0, 1.4843782, 0.0),
                vec3(0.0, 1.4790397, 0.0),
                vec3(0.0, 1.4737012, 0.0),
                vec3(0.0, 1.4672358, 0.0),
                vec3(0.0, 1.4607704, 0.0),
                vec3(0.0, 1.4534283, 0.0),
                vec3(0.0, 1.446086, 0.0),
                vec3(0.0, 1.4380066, 0.0),
                vec3(0.0, 1.4299271, 0.0),
                vec3(0.0, 1.4218477, 0.0),
                vec3(0.0, 1.4133819, 0.0),
                vec3(0.0, 1.4049162, 0.0),
                vec3(0.0, 1.3964503, 0.0),
                vec3(0.0, 1.3879845, 0.0),
                vec3(0.0, 1.3795187, 0.0),
                vec3(0.0, 1.371053, 0.0),
                vec3(0.0, 1.363187, 0.0),
                vec3(0.0, 1.3553208, 0.0),
                vec3(0.0, 1.3474548, 0.0),
                vec3(0.0, 1.3404669, 0.0),
                vec3(0.0, 1.3334789, 0.0),
                vec3(0.0, 1.327329, 0.0),
                vec3(0.0, 1.3211792, 0.0),
                vec3(0.0, 1.3159728, 0.0),
                vec3(0.0, 1.3113593, 0.0),
                vec3(0.0, 1.3073915, 0.0),
                vec3(0.0, 1.3042191, 0.0),
                vec3(0.0, 1.3016889, 0.0),
                vec3(0.0, 1.3001517, 0.0),
                vec3(0.0, 1.299547, 0.0),
                vec3(0.0, 1.3001189, 0.0),
                vec3(0.0, 1.301717, 0.0),
                vec3(0.0, 1.3040853, 0.0),
                vec3(0.0, 1.30725, 0.0),
                vec3(0.0, 1.3111147, 0.0),
                vec3(0.0, 1.3157693, 0.0),
                vec3(0.0, 1.3212559, 0.0),
                vec3(0.0, 1.3267424, 0.0),
                vec3(0.0, 1.3333225, 0.0),
                vec3(0.0, 1.3399025, 0.0),
                vec3(0.0, 1.3472242, 0.0),
                vec3(0.0, 1.354546, 0.0),
                vec3(0.0, 1.3626473, 0.0),
                vec3(0.0, 1.3707488, 0.0),
                vec3(0.0, 1.3788501, 0.0),
                vec3(0.0, 1.387325, 0.0),
                vec3(0.0, 1.3958, 0.0),
                vec3(0.0, 1.404275, 0.0),
                vec3(0.0, 1.4127499, 0.0),
                vec3(0.0, 1.4212248, 0.0),
                vec3(0.0, 1.4296998, 0.0),
                vec3(0.0, 1.4375246, 0.0),
                vec3(0.0, 1.4453492, 0.0),
                vec3(0.0, 1.453174, 0.0),
                vec3(0.0, 1.4601519, 0.0),
                vec3(0.0, 1.46713, 0.0),
                vec3(0.0, 1.4728534, 0.0),
                vec3(0.0, 1.478577, 0.0),
                vec3(0.0, 1.484428, 0.0),
                vec3(0.0, 1.489633, 0.0),
                vec3(0.0, 1.4934063, 0.0),
                vec3(0.0, 1.4957992, 0.0),
                vec3(0.0, 1.4975187, 0.0),
                vec3(0.0, 1.4988009, 0.0),
                vec3(0.0, 1.5, 0.0),
            ]),
            read_property_value_v12(&data, "test").unwrap()
        );
    }

    #[test]
    fn read_anim_v12_0834() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_jumpbgn_sht_gnd_fr.nuanmb, BASE, Translate
        let data = hex!(
            08340000
            0a000000
            0000803f
            0x4159883e
            00000000 df4f3d40 00000000
            00000000 0000c03f 00000000
            01000100 01000211
            ffff5b22 ff020011d 77fd311814bae46
            01000100 01000211
        );
        assert_values_close(
            V12Values::Vec3(vec![
                vec3(0.0, 2.9579999446868896, 0.0),
                vec3(0.0, 2.844062566757202, 0.0),
                vec3(0.0, 2.675625801086426, 0.0),
                vec3(0.0, 2.471597909927368, 0.0),
                vec3(0.0, 2.2501068115234375, 0.0),
                vec3(0.0, 2.028562545776367, 0.0),
                vec3(0.0, 1.8239102363586426, 0.0),
                vec3(0.0, 1.6552789211273193, 0.0),
                vec3(0.0, 1.5416259765625, 0.0),
                vec3(0.0, 1.5, 0.0),
            ]),
            read_property_value_v12(&data, "test").unwrap(),
        );
    }

    #[test]
    fn read_anim_v12_0934() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_kakubb01a_stk_air_fr.nuanmb, GBL_RT, Translate
        let data = hex!(
            09340000
            28000000
            0000803f
            0x51ec9d40
            0200
            0f00
            00000000 00000000 9f8ea33f
            00000000 00000000 f94f4a42
            00000000 00000000 0x6ea37042
            01000100 01000812
            01000100 01000812
            ffff2e0b 52170012 0180ba3692129e0b7f71393f1f29121d234316340d2a0722031dff18fc15fa12f80ff50d
            01000100 01000211
            01000100 01000211
            8a004419 ff002011 fd81578c
        );
        assert_eq!(
            V12Values::Vec3(vec![
                vec3(0.0, 0.0, 1.27779),
                vec3(0.0, 0.0, 2.5748847),
                vec3(0.0, 0.0, 3.8888035),
                vec3(0.0, 0.0, 5.2206244),
                vec3(0.0, 0.0, 6.569663),
                vec3(0.0, 0.0, 7.9355483),
                vec3(0.0, 0.0, 9.316959),
                vec3(0.0, 0.0, 10.713781),
                vec3(0.0, 0.0, 12.125164),
                vec3(0.0, 0.0, 13.550728),
                vec3(0.0, 0.0, 14.990418),
                vec3(0.0, 0.0, 16.442362),
                vec3(0.0, 0.0, 17.907713),
                vec3(0.0, 0.0, 19.384544),
                vec3(0.0, 0.0, 20.872856),
                vec3(0.0, 0.0, 22.372204),
                vec3(0.0, 0.0, 23.881601),
                vec3(0.0, 0.0, 25.40035),
                vec3(0.0, 0.0, 26.928825),
                vec3(0.0, 0.0, 28.465826),
                vec3(0.0, 0.0, 30.010803),
                vec3(0.0, 0.0, 31.563925),
                vec3(0.0, 0.0, 33.12375),
                vec3(0.0, 0.0, 34.68984),
                vec3(0.0, 0.0, 36.26108),
                vec3(0.0, 0.0, 37.838257),
                vec3(0.0, 0.0, 39.419827),
                vec3(0.0, 0.0, 41.0059),
                vec3(0.0, 0.0, 42.59557),
                vec3(0.0, 0.0, 44.188164),
                vec3(0.0, 0.0, 45.78313),
                vec3(0.0, 0.0, 47.380527),
                vec3(0.0, 0.0, 48.97861),
                vec3(0.0, 0.0, 50.5781),
                vec3(0.0, 0.0, 52.17822),
                vec3(0.0, 0.0, 53.776844),
                vec3(0.0, 0.0, 55.374786),
                vec3(0.0, 0.0, 56.972008),
                vec3(0.0, 0.0, 58.567272),
                vec3(0.0, 0.0, 60.1596),
            ]),
            read_property_value_v12(&data, "test").unwrap()
        );
    }

    #[test]
    fn read_anim_v12_0340() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_kakun31b_stk_air_fr.nuanmb, TE_R, Rotate
        let data = hex!(
            03400000
            00000000 00000000 f70435bf f704353f
        );
        assert_eq!(
            V12Values::Quat(vec![quat(0.0, 0.0, -0.707107, 0.707107)]),
            read_property_value_v12(&data, "test").unwrap()
        );
    }

    #[test]
    fn read_anim_v12_0043() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_kakun31b_stk_air_fr.nuanmb, TE_R, Rotate
        let data = hex!(
            00430000
            03000000
            0000803f
            00014000
            352a1c3f 3717b73e 0x9373023e a60e323f
            69c41c3f 0805b53e 4678fb3d e944323f
            69c41c3f 0805b53e 4678fb3d e944323f
        );
        assert_eq!(
            V12Values::Quat(vec![
                quat(0.6100191, 0.35759902, 0.12739402, 0.6955361),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
                quat(0.6123721, 0.3535541, 0.12278803, 0.69636416),
            ]),
            read_property_value_v12(&data, "test").unwrap()
        );
    }

    #[test]
    fn read_anim_v12_0843() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_bkmaintypebbgn_sht_gnd_bk.nuanmb, TE_R, Rotate
        let data = hex!(
            08430000
            06000000
            0000803f
            0005060708090000
            3945c13d
            e944323f 4678fb3d 4678fbbd e944323f
            eb004c3f 0f0e16be 8e5780bd 662d153f
            b3470100 ff010010 eb81eb07
            ffff0100 ff100010 a917ff7f
            bacb020e 57900100 ff01001032811a07
            483d0100 ff010010 7f57e4fc
        );
        assert_values_close(
            V12Values::Quat(vec![
                quat(
                    0.6963642239570618,
                    0.12278803437948227,
                    -0.12278803437948227,
                    0.6963642239570618,
                ),
                quat(
                    0.6963669657707214,
                    0.12278787791728973,
                    -0.12279731780290604,
                    0.6963598132133484,
                ),
                quat(
                    0.6963697671890259,
                    0.1227877140045166,
                    -0.1228065937757492,
                    0.696355402469635,
                ),
                quat(
                    0.6963725090026855,
                    0.12278755009174347,
                    -0.12281587719917297,
                    0.6963510513305664,
                ),
                quat(
                    0.69637531042099,
                    0.12278739362955093,
                    -0.12282515317201614,
                    0.696346640586853,
                ),
                quat(
                    0.6963780522346497,
                    0.1227872297167778,
                    -0.12283443659543991,
                    0.6963422894477844,
                ),
                quat(
                    0.7401219010353088,
                    0.03496725484728813,
                    -0.08779142051935196,
                    0.6657999157905579,
                ),
                quat(
                    0.7735146284103394,
                    -0.09664206206798553,
                    -0.03835618495941162,
                    0.6251913905143738,
                ),
                quat(
                    0.7829868793487549,
                    -0.15716108679771423,
                    -0.04652813822031021,
                    0.6000558733940125,
                ),
                quat(
                    0.796889066696167,
                    -0.14653801918029785,
                    -0.06266700476408005,
                    0.5827240347862244,
                ),
            ]),
            read_property_value_v12(&data, "test").unwrap(),
        );
    }

    #[test]
    fn read_anim_v12_0943() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_guardloop_stk_air_fr.nuanmb, HIZA_L, Rotate
        let data = hex!(
            09430000
            2e000000
            0000803f
            00030b0e101214161718191a1b1c1d1e1f2021222324252627292b2e3234363738393a3b3c3d3e3f4041424344450000
            178db83d
            0200
            1700
            00000000 00000000 2e765f3f 0x8ecef93e
            00000000 00000000 c1545f3f 3946fa3e
            00000000 00000000 e3545f3f b345fa3e
            01000100 01000812
            01000100 01000812
            908b1c19 4c062012 7fd0f5fa81f5abecdadeefddcba1c2bac3d2cadfd8dbecd587687878
            ffff5419 4a080012 81300b067f0b54142522102236603f473e2e37212825142c08220a0d0c0008fc
            01000100 01010211 00000000
            01000100 01010211 00000000
            9b089e0e ff010211 813e1005
            4f0f9f0e ff012011 7fc2f0fb158a469a
        );
        assert_values_close(
            V12Values::Quat(vec![
                quat(0.0, 0.0, 0.872897207736969, 0.4879041314125061),
                quat(0.0, 0.0, 0.873423159122467, 0.48696205019950867),
                quat(0.0, 0.0, 0.8739480376243591, 0.48601940274238586),
                quat(0.0, 0.0, 0.87447190284729, 0.4850761890411377),
                quat(0.0, 0.0, 0.8750185966491699, 0.4840892553329468),
                quat(0.0, 0.0, 0.8755642175674438, 0.4831017255783081),
                quat(0.0, 0.0, 0.8761087656021118, 0.4821135401725769),
                quat(0.0, 0.0, 0.8766521215438843, 0.48112475872039795),
                quat(0.0, 0.0, 0.8771944046020508, 0.48013538122177124),
                quat(0.0, 0.0, 0.8777355551719666, 0.4791453778743744),
                quat(0.0, 0.0, 0.8782755732536316, 0.4781547784805298),
                quat(0.0, 0.0, 0.8788145184516907, 0.47716355323791504),
                quat(0.0, 0.0, 0.879309892654419, 0.4762500822544098),
                quat(0.0, 0.0, 0.8798043131828308, 0.47533610463142395),
                quat(0.0, 0.0, 0.8802977800369263, 0.4744216203689575),
                quat(0.0, 0.0, 0.8807622194290161, 0.47355878353118896),
                quat(0.0, 0.0, 0.8812258243560791, 0.4726954698562622),
                quat(0.0, 0.0, 0.8816375732421875, 0.47192704677581787),
                quat(0.0, 0.0, 0.8820486664772034, 0.4711582660675049),
                quat(0.0, 0.0, 0.8824236392974854, 0.47045567631721497),
                quat(0.0, 0.0, 0.8827980160713196, 0.46975281834602356),
                quat(0.0, 0.0, 0.8831068873405457, 0.46917179226875305),
                quat(0.0, 0.0, 0.8834154605865479, 0.4685905873775482),
                quat(0.0, 0.0, 0.8836750388145447, 0.468100905418396),
                quat(0.0, 0.0, 0.8839226365089417, 0.46763312816619873),
                quat(0.0, 0.0, 0.884107768535614, 0.4672830104827881),
                quat(0.0, 0.0, 0.884317934513092, 0.46688517928123474),
                quat(0.0, 0.0, 0.8844209909439087, 0.46668997406959534),
                quat(0.0, 0.0, 0.8845252990722656, 0.466492235660553),
                quat(0.0, 0.0, 0.8845664262771606, 0.4664141833782196),
                quat(0.0, 0.0, 0.884566605091095, 0.4664139151573181),
                quat(0.0, 0.0, 0.884535014629364, 0.4664738178253174),
                quat(0.0, 0.0, 0.8844327926635742, 0.4666675627231598),
                quat(0.0, 0.0, 0.8842856287956238, 0.4669463634490967),
                quat(0.0, 0.0, 0.8840916752815247, 0.4673135578632355),
                quat(0.0, 0.0, 0.8838126063346863, 0.4678410291671753),
                quat(0.0, 0.0, 0.8835089802742004, 0.4684142768383026),
                quat(0.0, 0.0, 0.8831433653831482, 0.4691031277179718),
                quat(0.0, 0.0, 0.8827191591262817, 0.4699009656906128),
                quat(0.0, 0.0, 0.8822864294052124, 0.47071290016174316),
                quat(0.0, 0.0, 0.8817771077156067, 0.47166627645492554),
                quat(0.0, 0.0, 0.8812667727470398, 0.47261911630630493),
                quat(0.0, 0.0, 0.8807283043861389, 0.47362178564071655),
                quat(0.0, 0.0, 0.8801887035369873, 0.4746238589286804),
                quat(0.0, 0.0, 0.8795900344848633, 0.4757324159145355),
                quat(0.0, 0.0, 0.8789899945259094, 0.47684022784233093),
                quat(0.0, 0.0, 0.878388524055481, 0.47794726490974426),
                quat(0.0, 0.0, 0.8777865767478943, 0.47905194759368896),
                quat(0.0, 0.0, 0.877183198928833, 0.4801558554172516),
                quat(0.0, 0.0, 0.8765784502029419, 0.48125898838043213),
                quat(0.0, 0.0, 0.875972330570221, 0.482361376285553),
                quat(0.0, 0.0, 0.8753864765167236, 0.48342370986938477),
                quat(0.0, 0.0, 0.874799370765686, 0.48448535799980164),
                quat(0.0, 0.0, 0.8742668032646179, 0.4854457378387451),
                quat(0.0, 0.0, 0.8737331628799438, 0.48640555143356323),
                quat(0.0, 0.0, 0.873228132724762, 0.48731160163879395),
                quat(0.0, 0.0, 0.8728049993515015, 0.488069087266922),
                quat(0.0, 0.0, 0.8723865151405334, 0.4888167381286621),
                quat(0.0, 0.0, 0.872004508972168, 0.4894978404045105),
                quat(0.0, 0.0, 0.8717290163040161, 0.4899882376194),
                quat(0.0, 0.0, 0.8714519739151001, 0.49048084020614624),
                quat(0.0, 0.0, 0.8712426424026489, 0.49085259437561035),
                quat(0.0, 0.0, 0.8710882067680359, 0.49112656712532043),
                quat(0.0, 0.0, 0.8710342049598694, 0.4912223517894745),
                quat(0.0, 0.0, 0.8710559606552124, 0.4911837577819824),
                quat(0.0, 0.0, 0.8711516261100769, 0.49101412296295166),
                quat(0.0, 0.0, 0.871336042881012, 0.49068671464920044),
                quat(0.0, 0.0, 0.8715908527374268, 0.49023404717445374),
                quat(0.0, 0.0, 0.8719247579574585, 0.48963987827301025),
                quat(0.0, 0.0, 0.8723886609077454, 0.4888128340244293),
            ]),
            read_property_value_v12(&data, "test").unwrap(),
        );
    }

    #[test]
    fn read_anim_v12_0844() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_napalm22a_sht_air_fr.nuanmb, KUBI, Rotate
        let data = hex!(
            08440000
            19000000
            0000803f
            442a3b3c
            6f8dfe3c 0x28918ebc d0d90d3a 62d67f3f
            e620683d 49be0ebc 5ca5013a 31947f3f
            ffff9e09 81024012 46810a08547f175687cd86bc66bb66bb
            95386e15 5c002412 8f1dba8b
            5705f70c 29000612
            9208940d 78002412 71f16674
        );
        assert_values_close(
            V12Values::Quat(vec![
                quat(
                    0.0310733150690794,
                    -0.01740320771932602,
                    0.0005411182064563036,
                    0.9993654489517212,
                ),
                quat(
                    0.031552549451589584,
                    -0.017320076003670692,
                    0.0005391763988882303,
                    0.9993518590927124,
                ),
                quat(
                    0.032257866114377975,
                    -0.017034273594617844,
                    0.0005372439627535641,
                    0.9993342757225037,
                ),
                quat(
                    0.03323525935411453,
                    -0.016779618337750435,
                    0.0005353034939616919,
                    0.9993065595626831,
                ),
                quat(
                    0.03444314002990723,
                    -0.016495708376169205,
                    0.0005333604058250785,
                    0.9992703795433044,
                ),
                quat(
                    0.035833410918712616,
                    -0.01614263281226158,
                    0.0005314184818416834,
                    0.9992272257804871,
                ),
                quat(
                    0.037290122359991074,
                    -0.01571955531835556,
                    0.0005294801667332649,
                    0.9991806745529175,
                ),
                quat(
                    0.038893312215805054,
                    -0.015254848636686802,
                    0.0005275419098325074,
                    0.9991267919540405,
                ),
                quat(
                    0.04055259749293327,
                    -0.014779679477214813,
                    0.0005256031290628016,
                    0.9990679621696472,
                ),
                quat(
                    0.04226710647344589,
                    -0.014306616969406605,
                    0.0005236627184785903,
                    0.9990037679672241,
                ),
                quat(
                    0.04395878314971924,
                    -0.013828546740114689,
                    0.0005217224243097007,
                    0.9989374876022339,
                ),
                quat(
                    0.045617494732141495,
                    -0.013333922252058983,
                    0.0005197827122174203,
                    0.9988698363304138,
                ),
                quat(
                    0.04720192030072212,
                    -0.012822321616113186,
                    0.0005178438150323927,
                    0.9988029599189758,
                ),
                quat(
                    0.04871785268187523,
                    -0.01230636052787304,
                    0.0005159042193554342,
                    0.9987366199493408,
                ),
                quat(
                    0.0501178540289402,
                    -0.011800768785178661,
                    0.0005139636341482401,
                    0.9986734390258789,
                ),
                quat(
                    0.05141410604119301,
                    -0.011311073787510395,
                    0.0005120215937495232,
                    0.9986132383346558,
                ),
                quat(
                    0.0525432825088501,
                    -0.010834107175469398,
                    0.0005100803100503981,
                    0.9985597729682922,
                ),
                quat(
                    0.053524620831012726,
                    -0.01036971528083086,
                    0.0005081399576738477,
                    0.9985125660896301,
                ),
                quat(
                    0.05438004434108734,
                    -0.009931942448019981,
                    0.0005061996635049582,
                    0.9984707832336426,
                ),
                quat(
                    0.055102623999118805,
                    -0.009547295048832893,
                    0.0005042588454671204,
                    0.9984349012374878,
                ),
                quat(
                    0.05565357208251953,
                    -0.009239507839083672,
                    0.0005023180856369436,
                    0.998407244682312,
                ),
                quat(
                    0.05610117316246033,
                    -0.00901286955922842,
                    0.0005003760452382267,
                    0.9983842968940735,
                ),
                quat(
                    0.05640750750899315,
                    -0.008848452940583229,
                    0.000498435867484659,
                    0.998368501663208,
                ),
                quat(
                    0.056595537811517715,
                    -0.008716239593923092,
                    0.0004964989493601024,
                    0.9983590245246887,
                ),
                quat(
                    0.05667198449373245,
                    -0.008712357841432095,
                    0.0004945598775520921,
                    0.9983547329902649,
                ),
            ]),
            read_property_value_v12(&data, "test").unwrap(),
        );
    }

    #[test]
    fn read_anim_v12_0944() {
        // 001gundam_001gundam_001/001hito_001gundam_001gundam_001_guardloop_sht_air_fr.nuanmb, ASHI_L, Rotate
        let data = hex!(
            09440000
            46000000
            0000803f
            dab0cd3b
            0300
            1000
            20000000
            247d5abc e0200cbc 810a37be 27d87b3f
            7a7a1dbc 2278b6bb 69704bbe 7ae17a3f
            07c861bc a11813bc ce0037be f5d77b3f
            c5e25cbc c9580ebc 9ce136be cbd97b3f
            c8719109 63002612 fb219a74
            6769b609 62002612 f9218974
            ffffab22 4b026012 ff7fc40281fdc3ff648975896699679977997799
            9b2bd128 49002612 91f84588
            76607513 50002612 41fa6589
            49541e13 50002612 41fa6589
            bce9df2f 48026012 81b1510a7f1e3c10ab999b88a988a98899879987
            77279d34 49002612 1f2adb99
            79010100 ff010010 81d41f1a
            45010100 ff010010 81d5201a
            25050100 ff010010 7f2ce1e6
            e3000100 ff010010 7f2be0e6
        );
        assert_eq!(
            V12Values::Quat(vec![
                quat(-0.013335498, -0.008552759, -0.17875099, 0.98376685),
                quat(-0.013141736, -0.008489483, -0.17895888, 0.9837322),
                quat(-0.013003081, -0.008366773, -0.17932187, 0.983669),
                quat(-0.012843506, -0.00821765, -0.17969312, 0.9836046),
                quat(-0.012671822, -0.008052318, -0.18016055, 0.98352265),
                quat(-0.012497274, -0.007881654, -0.1806723, 0.9834324),
                quat(-0.01232608, -0.007713619, -0.18124439, 0.9833306),
                quat(-0.012160288, -0.0075517744, -0.18187602, 0.98321736),
                quat(-0.011998486, -0.0073957993, -0.18254046, 0.9830973),
                quat(-0.011837766, -0.007243483, -0.18326552, 0.9829655),
                quat(-0.011676286, -0.0070933, -0.18403819, 0.98282415),
                quat(-0.011514541, -0.0069458894, -0.18480875, 0.9826826),
                quat(-0.011354783, -0.006803781, -0.18559827, 0.9825367),
                quat(-0.011199581, -0.0066699954, -0.18642545, 0.9823827),
                quat(-0.011050063, -0.0065462184, -0.18726455, 0.9822256),
                quat(-0.010904852, -0.0064315945, -0.18811889, 0.9820647),
                quat(-0.010760771, -0.0063231112, -0.18898194, 0.9819013),
                quat(-0.01061474, -0.006217249, -0.18982157, 0.9817416),
                quat(-0.010465713, -0.006111901, -0.19064824, 0.98158365),
                quat(-0.010315883, -0.0060076737, -0.19147277, 0.98142534),
                quat(-0.010170145, -0.0059076785, -0.19228232, 0.98126924),
                quat(-0.01003402, -0.0058158548, -0.19308582, 0.98111343),
                quat(-0.009911364, -0.0057349266, -0.1938592, 0.9809625),
                quat(-0.009802959, -0.0056650243, -0.1945574, 0.9808258),
                quat(-0.009706752, -0.005603813, -0.19521394, 0.9806966),
                quat(-0.009620242, -0.0055484404, -0.1958594, 0.9805691),
                quat(-0.009543462, -0.005498139, -0.19645691, 0.9804507),
                quat(-0.0094806785, -0.0054559065, -0.19699948, 0.98034257),
                quat(-0.009440104, -0.0054284143, -0.19747749, 0.980247),
                quat(-0.0094311, -0.005423721, -0.197851, 0.9801718),
                quat(-0.009459667, -0.0054474683, -0.19816092, 0.9801087),
                quat(-0.009524761, -0.005499602, -0.19841935, 0.9800555),
                quat(-0.0096167065, -0.0055728806, -0.19858481, 0.98002064),
                quat(-0.009611724, -0.005568522, -0.19867107, 0.98000336),
                quat(-0.009697989, -0.005636047, -0.19865991, 0.9800043),
                quat(-0.009816655, -0.0057329736, -0.19853991, 0.9800269),
                quat(-0.00993086, -0.0058259754, -0.19831122, 0.9800715),
                quat(-0.010044327, -0.005918336, -0.19804272, 0.98012406),
                quat(-0.01016217, -0.006014469, -0.19766924, 0.9801977),
                quat(-0.010289464, -0.0061188024, -0.197233, 0.9802837),
                quat(-0.010430468, -0.0062350156, -0.19671172, 0.98038614),
                quat(-0.010587475, -0.0063650864, -0.19609773, 0.9805066),
                quat(-0.010760107, -0.006508735, -0.19547564, 0.98062795),
                quat(-0.010945995, -0.006663885, -0.19479267, 0.9807608),
                quat(-0.011141145, -0.0068271253, -0.1940979, 0.9808952),
                quat(-0.011341673, -0.006995077, -0.19333383, 0.98104256),
                quat(-0.011544539, -0.0071651256, -0.19251099, 0.98120075),
                quat(-0.011748168, -0.007335911, -0.19163743, 0.98136806),
                quat(-0.01195187, -0.0075068623, -0.19079632, 0.9815283),
                quat(-0.012155153, -0.007677518, -0.18998548, 0.9816817),
                quat(-0.012356552, -0.007846527, -0.18912801, 0.9818434),
                quat(-0.0125526255, -0.008010892, -0.1882522, 0.9820079),
                quat(-0.012738715, -0.008166551, -0.18735598, 0.9821756),
                quat(-0.012910123, -0.008309442, -0.18648784, 0.9823373),
                quat(-0.01306412, -0.00843716, -0.18562406, 0.9824977),
                quat(-0.013200727, -0.008549718, -0.18482284, 0.98264605),
                quat(-0.013323061, -0.008649738, -0.18403938, 0.9827905),
                quat(-0.013435416, -0.0087408805, -0.18327324, 0.9829313),
                quat(-0.013540838, -0.0088257585, -0.18254949, 0.9830638),
                quat(-0.013639043, -0.008904099, -0.18184108, 0.9831931),
                quat(-0.01372515, -0.008971735, -0.181187, 0.983312),
                quat(-0.013791077, -0.009021728, -0.18057641, 0.98342294),
                quat(-0.013828024, -0.009046614, -0.18011758, 0.98350626),
                quat(-0.01383129, -0.0090423105, -0.17964143, 0.98359346),
                quat(-0.013801866, -0.009009799, -0.17928039, 0.98366),
                quat(-0.013748859, -0.0089569315, -0.17895198, 0.983721),
                quat(-0.013780596, -0.008978037, -0.17871395, 0.98376364),
                quat(-0.013708919, -0.00890524, -0.17857687, 0.9837902),
                quat(-0.0136099225, -0.008809579, -0.17853504, 0.9838),
                quat(-0.0134818, -0.00868816, -0.178595, 0.983792),
            ]),
            read_property_value_v12(&data, "test").unwrap()
        );
    }
}
#[cfg(test)]
mod transform_flags_tests {
    use super::*;
    use ssbh_lib::formats::anim::{Property, TrackTypeV1, TrackV1};
    use ssbh_lib::{SsbhArray, SsbhByteBuffer, SsbhString};

    fn prop(name: &str, idx: u64) -> Property {
        Property {
            name: SsbhString::from(name),
            buffer_index: idx,
        }
    }

    #[test]
    fn missing_translate_sets_override_translation() {
        // Constant identity rotate 0x4003
        let mut rot = Vec::new();
        rot.extend_from_slice(&0x4003u32.to_le_bytes());
        rot.extend_from_slice(&0.0f32.to_le_bytes());
        rot.extend_from_slice(&0.0f32.to_le_bytes());
        rot.extend_from_slice(&0.0f32.to_le_bytes());
        rot.extend_from_slice(&1.0f32.to_le_bytes());
        let track = TrackV1 {
            name: SsbhString::from("BONE"),
            track_type: TrackTypeV1::Transform,
            properties: SsbhArray::from_vec(vec![prop("Rotate", 0)]),
        };
        let buffers = [SsbhByteBuffer::from_vec(rot)];
        let (_, _, flags) = read_track_values_v12(&track, &buffers, 1).unwrap();
        assert!(flags.override_translation);
        assert!(!flags.override_rotation);
        assert!(flags.override_scale);
    }
}
