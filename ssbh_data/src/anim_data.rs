//! Types for working with [Anim] data in .nuanmb files.
//!
//! # Examples
//! Animation data is stored in a hierarchy.
//! Values for each frame are stored at the [TrackData] level.
/*!
```rust no_run
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use ssbh_data::prelude::*;

let anim = AnimData::from_file("model.nuanmb")?;

for group in anim.groups {
    for node in group.nodes {
        for track in node.tracks {
            println!("Frame Count: {}", track.values.len());
        }
    }
}
# Ok(()) }
```
 */
//!
//! # Compression
//! Compressed animations use lossy compression for all data types except [TrackValues::Boolean].
//! Float compression encodes values using a configurable number of
//! values between two floating point endpoints.
//! Depending on the endpoints and number of bits, the encoded values
//! between the two endpoints may not be representable by 32 bit floating point.
//! This means that decompression may introduce some error, so compressing an animation
//! again with the same settings may produce slightly different compressed data.
//!
//! # File Differences
//! Unmodified files are not guaranteed to be binary identical after saving.
//! - For Anim v1.2, the default conversion path writes **uncompressed** buffers (constant /
//!   raw stream headers such as `0x3003`, `0x3300`, `0x4003`, `0x4300`) for VS2/wmmt2-style
//!   compatibility. Residual compression (`0x3409` / `0x4409`) is opt-in via
//!   [`AnimData::to_anim_v12_compressed`].
//! - For Anim v2.0+, compression may be enabled for a track if it would save space. This may
//!   produce differences with the original due to compression differences. These errors are small
//!   in practice but may cause gameplay differences such as online desyncs.
use binrw::BinRead;
use glam::{Quat, Vec3, Vec4};
use ssbh_lib::{Vector3, Vector4};
use ssbh_lib::{
    Version,
    formats::anim::{Anim, TrackTypeV2, TransformFlags as AnimTransformFlags},
};
use ssbh_write::SsbhWrite;
use std::{
    convert::{TryFrom, TryInto},
    error::Error,
};

pub use ssbh_lib::formats::anim::GroupType;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

mod bitutils;
pub mod error;
mod v1;
mod v2;

/// Data associated with an [Anim] file.
/// Supported versions are 1.2, 2.0, and 2.1.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Clone)]
pub struct AnimData {
    pub major_version: u16,
    pub minor_version: u16,

    /// The index of the last frame in the animation,
    /// which is calculated as `(frame_count - 1) as f32`.
    ///
    /// Constant animations will last for final_frame_index + 1 many frames.
    ///
    /// Frames use floating point to allow the rendering speed to differ from the animation speed.
    /// For example, some animations in Smash Ultimate interpolate when playing the game at 60fps but 1/4 speed.
    pub final_frame_index: f32,
    pub groups: Vec<GroupData>,
}

impl AnimData {
    /// Encode all animation data to the specified version.
    ///
    /// For Anim **v1.2**, this uses the **uncompressed** writer (wmmt2 / VS2 default):
    /// constant and raw-stream property buffers only — no residual `0x3409` / `0x4409`.
    /// Prefer [`AnimData::to_anim_v12_compressed`] when you explicitly need residual
    /// compression for EXVS2-style multi-frame tracks.
    ///
    /// For Anim **v2.0+**, compression is chosen by the encoder when it saves space.
    pub fn to_anim(&self) -> Result<Anim, error::Error> {
        match (self.major_version, self.minor_version) {
            // Default v1.2: uncompressed (wmmt2 policy). Residual is opt-in.
            (1, 2) => Ok(v1::create_anim_v12_uncompressed(self)?),
            (2, 0) => v2::create_anim_v20(self),
            (2, 1) => v2::create_anim_v21(self),
            (major_version, minor_version) => Err(error::Error::UnsupportedVersion {
                major_version,
                minor_version,
            }),
        }
    }

    /// Encode Anim v1.2 using only constant / raw-stream property formats
    /// (no residual `0x3409` / `0x4409`).
    ///
    /// For `(1, 2)` this matches [`AnimData::to_anim`]. Other versions are not supported.
    pub fn to_anim_uncompressed(&self) -> Result<Anim, error::Error> {
        match (self.major_version, self.minor_version) {
            (1, 2) => Ok(v1::create_anim_v12_uncompressed(self)?),
            (major_version, minor_version) => Err(error::Error::UnsupportedVersion {
                major_version,
                minor_version,
            }),
        }
    }

    /// Encode Anim v1.2 with EXVS2-style residual compression for multi-frame
    /// Transform tracks (`0x3409` Vector3, `0x4409` quaternion).
    ///
    /// Opt-in only: the default [`AnimData::to_anim`] path is uncompressed.
    pub fn to_anim_v12_compressed(&self) -> Result<Anim, error::Error> {
        match (self.major_version, self.minor_version) {
            (1, 2) => Ok(v1::create_anim_v12(self)?),
            (major_version, minor_version) => Err(error::Error::UnsupportedVersion {
                major_version,
                minor_version,
            }),
        }
    }
}

// TODO: Test these conversions.
impl TryFrom<Anim> for AnimData {
    type Error = Box<dyn Error>;

    fn try_from(anim: Anim) -> Result<Self, Self::Error> {
        (&anim).try_into()
    }
}

impl TryFrom<&Anim> for AnimData {
    type Error = Box<dyn Error>;

    fn try_from(anim: &Anim) -> Result<Self, Self::Error> {
        let (major_version, minor_version) = anim.major_minor_version();
        Ok(Self {
            major_version,
            minor_version,
            final_frame_index: match &anim {
                Anim::V12 {
                    final_frame_index,
                    unk1,
                    unk2,
                    ..
                } => v12_effective_final_frame_index(*final_frame_index, *unk1, *unk2),
                Anim::V20 {
                    final_frame_index, ..
                } => *final_frame_index,
                Anim::V21 {
                    final_frame_index, ..
                } => *final_frame_index,
            },
            groups: read_anim_groups(anim)?,
        })
    }
}

/// Resolve the high-level last-frame index for Anim v1.2 (VS2/EXVS2-focused).
///
/// Two on-disk conventions are observed:
/// - Smash Ultimate style: `final_frame_index` is the last frame index (`frame_count - 1`).
/// - EXVS2 / VS2 style: `final_frame_index` is a timebase (commonly `60.0`) and `unk2` holds
///   the effective end frame (`frame_count - 1`).
///
/// Detection is deliberately stricter than `final≈60 && unk2>=0` so a Smash-style
/// 61-frame clip (`final=60`, `unk2=0`, `unk1=1`) is not truncated to one frame.
fn v12_effective_final_frame_index(final_frame_index: f32, unk1: f32, unk2: f32) -> f32 {
    if (final_frame_index - 60.0).abs() > 1.0e-3 {
        return final_frame_index;
    }

    // Timebase field is ~60. Prefer EXVS2 when unk2 is a positive end frame.
    if unk2 > 0.0 {
        return unk2;
    }

    // unk2 == 0 with timebase 60:
    // - EXVS2 single-frame / zero end often has duration unk1 ≈ 0
    // - Smash-style end-at-60 commonly keeps unk1 as a non-zero multiplier (often 1.0)
    if unk1.abs() <= 1.0e-3 {
        0.0
    } else {
        final_frame_index
    }
}

/// Frame count used when decoding Anim v1.2 track buffers.
fn v12_frame_count(final_frame_index: f32, unk1: f32, unk2: f32) -> usize {
    let end = v12_effective_final_frame_index(final_frame_index, unk1, unk2);
    end.round().max(0.0) as usize + 1
}

impl TryFrom<AnimData> for Anim {
    type Error = error::Error;

    fn try_from(data: AnimData) -> Result<Self, Self::Error> {
        data.to_anim()
    }
}

impl TryFrom<&AnimData> for Anim {
    type Error = error::Error;

    fn try_from(data: &AnimData) -> Result<Self, Self::Error> {
        data.to_anim()
    }
}

/// Data associated with a [Group][ssbh_lib::formats::anim::Group].
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Clone)]
pub struct GroupData {
    /// The usage type for all the [NodeData] in [nodes](#structfield.nodes)
    pub group_type: GroupType,
    pub nodes: Vec<NodeData>,
}

/// Data associated with a [Node][ssbh_lib::formats::anim::Node].
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Clone)]
pub struct NodeData {
    pub name: String,
    pub tracks: Vec<TrackData>,
}

/// The data associated with a [TrackV2](ssbh_lib::formats::anim::TrackV2).
///
/// # Examples
/// The scale settings and transform flags should usually use their default value.
/**
```rust
use ssbh_data::anim_data::{TrackData, TrackValues, Transform, TransformFlags};

let track = TrackData {
    name: "Transform".to_string(),
    values: TrackValues::Transform(vec![Transform::IDENTITY]),
    compensate_scale: false,
    transform_flags: TransformFlags::default()
};
```
 */
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Clone)]
pub struct TrackData {
    /// The name of the property to animate.
    ///
    /// For tracks in a group of type [GroupType::Material], this is the name of the material parameter like "CustomVector31".
    /// Other group types tend to use the name of the group type like "Transform" or "Visibility".
    pub name: String,

    /// Revert the scaling of the immediate parent when `true`.
    /// Only applies to [TrackValues::Transform].
    ///
    /// The final scale relative to the parent is `current_scale * (1 / parent_scale)`.
    /// For Smash Ultimate, this is not applied recursively on the parent,
    /// so only the immediate parent's scaling is taken into account.
    /// This matches the behavior of scale compensation in Autodesk Maya.
    pub compensate_scale: bool,

    pub transform_flags: TransformFlags,

    /// The frame values for the property specified by [name](#structfield.name).
    ///
    /// Each element in the [TrackValues] provides the value for a single frame.
    /// If the [TrackValues] contains a single element, this track will be considered constant
    /// and repeat that element for each frame in the animation
    /// up to and including [final_frame_index](struct.AnimData.html#structfield.final_frame_index).
    pub values: TrackValues,
}

/// See [ssbh_lib::formats::anim::TransformFlags].
// Including compensate scale would be redundant with ScaleOptions.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub struct TransformFlags {
    pub override_translation: bool,
    pub override_rotation: bool,
    pub override_scale: bool,
    pub override_compensate_scale: bool,
}

impl From<TransformFlags> for AnimTransformFlags {
    fn from(f: TransformFlags) -> Self {
        Self::new(
            f.override_translation,
            f.override_rotation,
            f.override_scale,
            f.override_compensate_scale,
        )
    }
}

impl From<AnimTransformFlags> for TransformFlags {
    fn from(f: AnimTransformFlags) -> Self {
        Self {
            override_translation: f.override_translation(),
            override_rotation: f.override_rotation(),
            override_scale: f.override_scale(),
            override_compensate_scale: f.override_compensate_scale(),
        }
    }
}

// TODO: Investigate if the names based on the Anim 1.2 property names are accurate.
/// A decomposed 2D transformation for texture coordinates.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, BinRead, PartialEq, SsbhWrite, Default, Clone, Copy)]
pub struct UvTransform {
    pub scale_u: f32,
    pub scale_v: f32,
    pub rotation: f32,
    pub translate_u: f32,
    pub translate_v: f32,
}

/// A decomposed 3D transformation consisting of a scale, rotation, and translation.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct Transform {
    pub scale: Vec3,
    pub rotation: Quat,
    pub translation: Vec3,
}

impl Transform {
    /// An identity transformation representing no scale, rotation, or translation.
    pub const IDENTITY: Transform = Transform {
        scale: Vec3::ONE,
        rotation: Quat::IDENTITY,
        translation: Vec3::ZERO,
    };
}

// TODO: Add version 1.2 types.
// TODO: Create runtime errors when saving tracks with incompatible data?
/// A value collection with an element for each frame of the animation.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Clone)]
pub enum TrackValues {
    /// Transformations used for camera or skeletal animations.
    Transform(Vec<Transform>),
    /// Transformations applied to UV coordinates for texture animations.
    UvTransform(Vec<UvTransform>),
    /// Animated scalar parameter values.
    Float(Vec<f32>),
    // TODO: rename to u32?
    PatternIndex(Vec<u32>),
    /// Visibility animations or animated boolean parameters.
    Boolean(Vec<bool>),
    /// Material animations or animated vector parameters.
    Vector4(Vec<Vec4>),
}

impl TrackValues {
    /// Returns the number of elements, which is equivalent to the number of frames.
    /// # Examples
    /**
    ```rust
    # use ssbh_data::anim_data::TrackValues;
    assert_eq!(3, TrackValues::Boolean(vec![true, false, true]).len());
    ```
     */
    pub fn len(&self) -> usize {
        match self {
            TrackValues::Transform(v) => v.len(),
            TrackValues::UvTransform(v) => v.len(),
            TrackValues::Float(v) => v.len(),
            TrackValues::PatternIndex(v) => v.len(),
            TrackValues::Boolean(v) => v.len(),
            TrackValues::Vector4(v) => v.len(),
        }
    }

    /// Returns `true` there are no elements.
    /**
    ```rust
    # use ssbh_data::anim_data::TrackValues;
    assert!(TrackValues::Transform(Vec::new()).is_empty());
    ```
     */
    pub fn is_empty(&self) -> bool {
        match self {
            TrackValues::Transform(v) => v.is_empty(),
            TrackValues::UvTransform(v) => v.is_empty(),
            TrackValues::Float(v) => v.is_empty(),
            TrackValues::PatternIndex(v) => v.is_empty(),
            TrackValues::Boolean(v) => v.is_empty(),
            TrackValues::Vector4(v) => v.is_empty(),
        }
    }

    fn track_type(&self) -> TrackTypeV2 {
        match self {
            TrackValues::Transform(_) => TrackTypeV2::Transform,
            TrackValues::UvTransform(_) => TrackTypeV2::UvTransform,
            TrackValues::Float(_) => TrackTypeV2::Float,
            TrackValues::PatternIndex(_) => TrackTypeV2::PatternIndex,
            TrackValues::Boolean(_) => TrackTypeV2::Boolean,
            TrackValues::Vector4(_) => TrackTypeV2::Vector4,
        }
    }
}

// TODO: Test conversions from anim?
fn read_anim_groups(anim: &Anim) -> Result<Vec<GroupData>, error::Error> {
    match anim {
        ssbh_lib::prelude::Anim::V12 {
            tracks,
            buffers,
            final_frame_index,
            unk1,
            unk2,
            ..
        } => {
            // Prefer EXVS2 dual-header convention when present (timebase 60 + unk2 end).
            let frame_count = v12_frame_count(*final_frame_index, *unk1, *unk2);
            v1::read_groups_v12(&tracks.elements, &buffers.elements, frame_count)
        }
        ssbh_lib::formats::anim::Anim::V20 { groups, buffer, .. } => {
            v2::read_groups_v20(&groups.elements, &buffer.elements)
        }
        ssbh_lib::formats::anim::Anim::V21 { groups, buffer, .. } => {
            v2::read_groups_v20(&groups.elements, &buffer.elements)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Test the conversions more thoroughly.

    #[test]
    fn create_empty_anim_v_1_2() {
        let anim = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: 1.5,
            groups: Vec::new(),
        }
        .to_anim()
        .unwrap();

        // EXVS2 dual-header: timebase 60, end frame in unk2, duration in unk1.
        assert!(matches!(
            anim,
            Anim::V12 {
                unk1,
                final_frame_index,
                unk2,
                unk3,
                ..
            } if (unk1 - 1.5 / 60.0).abs() < 1e-6
                && final_frame_index == 60.0
                && unk2 == 1.5
                && unk3 == 0.0
        ));
    }

    #[test]
    fn create_empty_anim_v_1_2_uncompressed() {
        let anim = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: 1.5,
            groups: Vec::new(),
        }
        .to_anim_uncompressed()
        .unwrap();

        assert!(matches!(
            anim,
            Anim::V12 {
                unk1,
                final_frame_index,
                unk2,
                unk3,
                ..
            } if (unk1 - 1.5 / 60.0).abs() < 1e-6
                && final_frame_index == 60.0
                && unk2 == 1.5
                && unk3 == 0.0
        ));
    }

    #[test]
    fn create_empty_anim_v_2_0() {
        let anim = AnimData {
            major_version: 2,
            minor_version: 0,
            final_frame_index: 1.5,
            groups: Vec::new(),
        }
        .to_anim()
        .unwrap();

        assert!(matches!(
            anim,
            Anim::V20 {
                final_frame_index,
                ..
            } if final_frame_index == 1.5
        ));
    }

    #[test]
    fn create_empty_anim_v_2_1() {
        let anim = AnimData {
            major_version: 2,
            minor_version: 1,
            final_frame_index: 2.5,
            groups: Vec::new(),
        }
        .to_anim()
        .unwrap();

        assert!(matches!(anim, Anim::V21 {
            final_frame_index,
            ..
        } if final_frame_index == 2.5));
    }

    #[test]
    fn create_anim_negative_frame_index() {
        let result = AnimData {
            major_version: 2,
            minor_version: 1,
            final_frame_index: -1.0,
            groups: Vec::new(),
        }
        .to_anim();

        assert!(matches!(
            result,
            Err(error::Error::InvalidFinalFrameIndex {
                final_frame_index
            }) if final_frame_index == -1.0
        ));
    }

    #[test]
    fn create_anim_insufficient_frame_index() {
        let result = AnimData {
            major_version: 2,
            minor_version: 1,
            final_frame_index: 2.0,
            groups: vec![GroupData {
                group_type: GroupType::Visibility,
                nodes: vec![NodeData {
                    name: String::new(),
                    tracks: vec![TrackData {
                        name: String::new(),
                        values: TrackValues::Boolean(vec![true; 4]),
                        compensate_scale: false,
                        transform_flags: TransformFlags::default(),
                    }],
                }],
            }],
        }
        .to_anim();

        // A value of at least 3.0 is expected.
        assert!(matches!(
            result,
            Err(error::Error::InvalidFinalFrameIndex {
                final_frame_index
            }) if final_frame_index == 2.0
        ));
    }

    #[test]
    fn create_anim_zero_frame_index() {
        let anim = AnimData {
            major_version: 2,
            minor_version: 1,
            final_frame_index: 0.0,
            groups: Vec::new(),
        }
        .to_anim()
        .unwrap();

        assert!(matches!(anim, Anim::V21 {
            final_frame_index,
            ..
        } if final_frame_index == 0.0));
    }

    #[test]
    fn create_empty_anim_invalid_version() {
        let result = AnimData {
            major_version: 1,
            minor_version: 1,
            final_frame_index: 0.0,
            groups: Vec::new(),
        }
        .to_anim();

        assert!(matches!(
            result,
            Err(error::Error::UnsupportedVersion {
                major_version: 1,
                minor_version: 1
            })
        ));
    }

    /// Public nuanmb path: build v1.2 AnimData, encode via default `to_anim` (uncompressed),
    /// then decode back through `AnimData::try_from` (shipped entry points only).
    #[test]
    fn nuanmb_v12_visibility_round_trip_public_api() {
        let original = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: 2.0,
            groups: vec![GroupData {
                group_type: GroupType::Visibility,
                nodes: vec![NodeData {
                    name: "Mesh".into(),
                    tracks: vec![TrackData {
                        name: "Visibility".into(),
                        values: TrackValues::Boolean(vec![true, false, true]),
                        compensate_scale: false,
                        transform_flags: TransformFlags::default(),
                    }],
                }],
            }],
        };

        let anim = original.to_anim().expect("encode v1.2 via public to_anim");
        assert!(matches!(anim, Anim::V12 { .. }));

        let decoded = AnimData::try_from(&anim).expect("decode via public TryFrom");
        assert_eq!(1, decoded.major_version);
        assert_eq!(2, decoded.minor_version);
        assert_eq!(1, decoded.groups.len());
        assert_eq!(GroupType::Visibility, decoded.groups[0].group_type);
        assert_eq!("Mesh", decoded.groups[0].nodes[0].name);
        match &decoded.groups[0].nodes[0].tracks[0].values {
            TrackValues::Boolean(values) => assert_eq!(vec![true, false, true], *values),
            other => panic!("expected boolean track, got {other:?}"),
        }
    }

    #[test]
    fn nuanmb_v12_uncompressed_public_api() {
        let original = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: 1.0,
            groups: vec![GroupData {
                group_type: GroupType::Visibility,
                nodes: vec![NodeData {
                    name: "Obj".into(),
                    tracks: vec![TrackData {
                        name: "Visibility".into(),
                        values: TrackValues::Boolean(vec![false, true]),
                        compensate_scale: false,
                        transform_flags: TransformFlags::default(),
                    }],
                }],
            }],
        };

        let anim = original
            .to_anim_uncompressed()
            .expect("encode uncompressed v1.2");
        let decoded = AnimData::try_from(&anim).expect("decode uncompressed v1.2");
        match &decoded.groups[0].nodes[0].tracks[0].values {
            TrackValues::Boolean(values) => assert_eq!(vec![false, true], *values),
            other => panic!("expected boolean track, got {other:?}"),
        }
    }

    /// Default `to_anim` for v1.2 must use uncompressed property headers
    /// (no residual 0x3409 / 0x4409), matching wmmt2 policy.
    #[test]
    fn nuanmb_v12_default_to_anim_uses_uncompressed_headers() {
        let frames: Vec<Transform> = (0..4)
            .map(|i| {
                let t = i as f32;
                Transform {
                    scale: Vec3::new(1.0 + t * 0.01, 1.0, 1.0),
                    rotation: Quat::from_xyzw(0.0, 0.0, (t * 0.05).sin(), (t * 0.05).cos())
                        .normalize(),
                    translation: Vec3::new(t, 0.0, t * 2.0),
                }
            })
            .collect();

        let original = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: 3.0,
            groups: vec![GroupData {
                group_type: GroupType::Transform,
                nodes: vec![NodeData {
                    name: "Bone".into(),
                    tracks: vec![TrackData {
                        name: "Transform".into(),
                        values: TrackValues::Transform(frames),
                        compensate_scale: false,
                        transform_flags: TransformFlags::default(),
                    }],
                }],
            }],
        };

        let anim = original.to_anim().expect("default to_anim v1.2");
        let Anim::V12 { ref buffers, .. } = anim else {
            panic!("expected Anim::V12");
        };

        let headers: Vec<u32> = buffers
            .elements
            .iter()
            .map(|b| {
                assert!(b.elements.len() >= 4, "buffer too small");
                u32::from_le_bytes(b.elements[0..4].try_into().unwrap())
            })
            .collect();

        // Residual compression must not appear on the default path.
        assert!(
            !headers.contains(&0x3409) && !headers.contains(&0x4409),
            "default write must not use residual 0x3409/0x4409, got {headers:?}"
        );
        // Multi-frame varying scale/translate → 0x3300; rotate → 0x4300.
        assert!(
            headers.iter().any(|h| matches!(h, 0x3003 | 0x3300 | 0x3400)),
            "expected uncompressed Vector3 headers, got {headers:?}"
        );
        assert!(
            headers.iter().any(|h| matches!(h, 0x4003 | 0x4300)),
            "expected uncompressed rotation headers, got {headers:?}"
        );

        let decoded = AnimData::try_from(&anim).expect("decode default uncompressed v1.2");
        match &decoded.groups[0].nodes[0].tracks[0].values {
            TrackValues::Transform(values) => assert_eq!(4, values.len()),
            other => panic!("expected transform track, got {other:?}"),
        }
    }

    /// Explicit residual writer still available via `to_anim_v12_compressed`.
    #[test]
    fn nuanmb_v12_compressed_opt_in_uses_residual_headers() {
        let frames: Vec<Transform> = (0..4)
            .map(|i| {
                let t = i as f32;
                Transform {
                    scale: Vec3::new(1.0 + t * 0.1, 1.0 + t * 0.05, 1.0),
                    rotation: Quat::from_xyzw(0.0, t * 0.1, 0.0, 1.0).normalize(),
                    translation: Vec3::new(t, t * 0.5, t * 2.0),
                }
            })
            .collect();

        let original = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: 3.0,
            groups: vec![GroupData {
                group_type: GroupType::Transform,
                nodes: vec![NodeData {
                    name: "Bone".into(),
                    tracks: vec![TrackData {
                        name: "Transform".into(),
                        values: TrackValues::Transform(frames),
                        compensate_scale: false,
                        transform_flags: TransformFlags::default(),
                    }],
                }],
            }],
        };

        let anim = original
            .to_anim_v12_compressed()
            .expect("opt-in residual encode");
        let Anim::V12 { buffers, .. } = anim else {
            panic!("expected Anim::V12");
        };

        let headers: Vec<u32> = buffers
            .elements
            .iter()
            .map(|b| u32::from_le_bytes(b.elements[0..4].try_into().unwrap()))
            .collect();
        assert!(
            headers.contains(&0x3409) || headers.contains(&0x4409),
            "compressed path should emit residual 0x3409 and/or 0x4409, got {headers:?}"
        );
    }

    /// EXVS2 V12 on-disk header: timebase 60 in `final_frame_index`, end frame in `unk2`.
    /// High-level `AnimData.final_frame_index` must remain the effective end frame.
    #[test]
    fn nuanmb_v12_exvs2_header_write_via_to_anim() {
        let data = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: 39.0, // end frame (frame_count - 1)
            groups: Vec::new(),
        };

        let anim = data.to_anim().expect("to_anim v1.2");
        match anim {
            Anim::V12 {
                unk1,
                final_frame_index,
                unk2,
                unk3,
                ..
            } => {
                assert!(
                    (unk1 - 39.0 / 60.0).abs() < 1e-6,
                    "unk1 should be duration seconds (end/60), got {unk1}"
                );
                assert_eq!(60.0, final_frame_index, "file final_frame_index is timebase");
                assert_eq!(39.0, unk2, "unk2 holds effective end frame");
                assert_eq!(0.0, unk3);
            }
            other => panic!("expected Anim::V12, got {other:?}"),
        }
    }

    /// Reading an EXVS2-style V12 must map unk2 → AnimData.final_frame_index and
    /// must not treat the 60.0 timebase as a 61-frame timeline.
    #[test]
    fn nuanmb_v12_exvs2_header_read_via_try_from() {
        let anim = Anim::V12 {
            name: "".into(),
            unk1: 0.65, // 39/60
            final_frame_index: 60.0,
            unk2: 39.0,
            unk3: 0.0,
            tracks: ssbh_lib::SsbhArray::new(),
            buffers: ssbh_lib::SsbhArray::new(),
        };

        let data = AnimData::try_from(&anim).expect("try_from EXVS2 V12");
        assert_eq!(1, data.major_version);
        assert_eq!(2, data.minor_version);
        assert_eq!(
            39.0, data.final_frame_index,
            "EXVS2: high-level end frame comes from unk2, not the 60.0 timebase"
        );
        assert!(data.groups.is_empty());
    }

    /// Smash Ultimate-style V12 (final_frame_index is the end frame, not ~60) still works.
    #[test]
    fn nuanmb_v12_smash_style_header_read_via_try_from() {
        let anim = Anim::V12 {
            name: "".into(),
            unk1: 1.0,
            final_frame_index: 12.0,
            unk2: 0.0,
            unk3: 0.0,
            tracks: ssbh_lib::SsbhArray::new(),
            buffers: ssbh_lib::SsbhArray::new(),
        };

        let data = AnimData::try_from(&anim).expect("try_from Smash-style V12");
        assert_eq!(12.0, data.final_frame_index);
    }

    /// Smash-style end-at-60 must not be truncated by the EXVS2 dual-header heuristic.
    #[test]
    fn nuanmb_v12_smash_style_final_60_unk2_zero_keeps_end_frame() {
        let anim = Anim::V12 {
            name: "".into(),
            unk1: 1.0,
            final_frame_index: 60.0,
            unk2: 0.0,
            unk3: 0.0,
            tracks: ssbh_lib::SsbhArray::new(),
            buffers: ssbh_lib::SsbhArray::new(),
        };
        let data = AnimData::try_from(&anim).expect("try_from Smash final=60");
        assert_eq!(60.0, data.final_frame_index);
    }

    #[test]
    fn nuanmb_v12_uv_transform_round_trip_public_api() {
        let original = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: 1.0,
            groups: vec![GroupData {
                group_type: GroupType::Material,
                nodes: vec![NodeData {
                    name: "Mat".into(),
                    tracks: vec![TrackData {
                        name: "UvTransform".into(),
                        values: TrackValues::UvTransform(vec![
                            UvTransform {
                                scale_u: 1.0,
                                scale_v: 1.0,
                                rotation: 0.0,
                                translate_u: 0.0,
                                translate_v: 0.0,
                            },
                            UvTransform {
                                scale_u: 2.0,
                                scale_v: 0.5,
                                rotation: 0.25,
                                translate_u: 0.1,
                                translate_v: -0.2,
                            },
                        ]),
                        compensate_scale: false,
                        transform_flags: TransformFlags::default(),
                    }],
                }],
            }],
        };

        let anim = original.to_anim().expect("encode UV v1.2");
        let decoded = AnimData::try_from(&anim).expect("decode UV v1.2");
        match &decoded.groups[0].nodes[0].tracks[0].values {
            TrackValues::UvTransform(values) => {
                assert_eq!(2, values.len());
                assert!((values[1].scale_u - 2.0).abs() < 1e-5);
                assert!((values[1].translate_v - -0.2).abs() < 1e-5);
            }
            other => panic!("expected UV track, got {other:?}"),
        }
    }

    #[test]
    fn nuanmb_v12_unknown_property_header_returns_error_not_panic() {
        use ssbh_lib::formats::anim::{Property, TrackTypeV1, TrackV1};
        use ssbh_lib::SsbhByteBuffer;

        let mut bad = Vec::new();
        bad.extend_from_slice(&0xDEADu32.to_le_bytes());
        let anim = Anim::V12 {
            name: "".into(),
            unk1: 0.0,
            final_frame_index: 60.0,
            unk2: 0.0,
            unk3: 0.0,
            tracks: vec![TrackV1 {
                name: "x".into(),
                track_type: TrackTypeV1::Visibility,
                properties: vec![Property {
                    name: "Visibility".into(),
                    buffer_index: 0,
                }]
                .into(),
            }]
            .into(),
            buffers: vec![SsbhByteBuffer { elements: bad }].into(),
        };

        let result = AnimData::try_from(&anim);
        assert!(result.is_err(), "unknown header must fail closed: {result:?}");
        let err = result.err().unwrap().to_string();
        assert!(
            err.to_lowercase().contains("unsupported") || err.contains("DEAD"),
            "error should mention unsupported header, got: {err}"
        );
    }

    /// Full public round-trip: high-level end frame → EXVS2 headers → back to end frame,
    /// with track lengths driven by the EXVS2 frame_count (unk2+1), not 61.
    #[test]
    fn nuanmb_v12_exvs2_header_and_visibility_round_trip() {
        let end_frame = 2.0;
        let original = AnimData {
            major_version: 1,
            minor_version: 2,
            final_frame_index: end_frame,
            groups: vec![GroupData {
                group_type: GroupType::Visibility,
                nodes: vec![NodeData {
                    name: "Mesh".into(),
                    tracks: vec![TrackData {
                        name: "Visibility".into(),
                        values: TrackValues::Boolean(vec![true, false, true]),
                        compensate_scale: false,
                        transform_flags: TransformFlags::default(),
                    }],
                }],
            }],
        };

        let anim = original.to_anim().expect("encode");
        match &anim {
            Anim::V12 {
                final_frame_index,
                unk2,
                ..
            } => {
                assert_eq!(60.0, *final_frame_index);
                assert_eq!(end_frame, *unk2);
            }
            other => panic!("expected V12, got {other:?}"),
        }

        let decoded = AnimData::try_from(&anim).expect("decode");
        assert_eq!(end_frame, decoded.final_frame_index);
        match &decoded.groups[0].nodes[0].tracks[0].values {
            TrackValues::Boolean(values) => {
                assert_eq!(
                    3,
                    values.len(),
                    "frame_count must be unk2+1 (=3), not timebase+1 (=61)"
                );
                assert_eq!(vec![true, false, true], *values);
            }
            other => panic!("expected boolean track, got {other:?}"),
        }
    }
}
