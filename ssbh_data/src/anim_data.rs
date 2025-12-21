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
//! Anim v2.0+ may use lossy compression for most data types except [TrackValues::Boolean].
//! Float compression encodes values using a configurable number of values between two floating
//! point endpoints. Depending on the endpoints and number of bits, the encoded values between the
//! two endpoints may not be representable by 32 bit floating point.
//!
//! This means decompression may introduce some error, so compressing an animation again with the
//! same settings may produce slightly different compressed data.
//!
//! # File Differences
//! Unmodified files are not guaranteed to be binary identical after saving.
//!
//! - For Anim v1.2, the default conversion path writes uncompressed buffers for simplicity and
//!   compatibility. Use `AnimData::to_anim_v12_compressed()` to explicitly request EXVS2-style
//!   compression (e.g. 0x3409/0x4409).
//! - For Anim v2.0+, compression may be enabled for a track if it would save space. This may
//!   produce differences with the original due to compression differences. These errors are small
//!   in practice but may cause gameplay differences such as online desyncs.
use binrw::io::{Cursor, Seek, Write};
use binrw::{BinRead, BinReaderExt};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
pub use ssbh_lib::formats::anim::GroupType;
use ssbh_lib::formats::anim::TrackTypeV1;
use ssbh_lib::{
    formats::anim::{
        Anim, CompressionType, Group, Node, TrackFlags, TrackTypeV2, TrackV2,
        TransformFlags as AnimTransformFlags, UnkData,
    },
    SsbhArray, Vector3, Vector4, Version,
};
use ssbh_write::SsbhWrite;
use std::collections::HashMap;
use std::{
    convert::{TryFrom, TryInto},
    error::Error,
};

mod buffers;
use buffers::*;
mod bitutils;
mod compression;
#[path = "anim_create_v12.rs"]
mod anim_create_v12;
#[path = "nuanmb_v12/mod.rs"]
pub mod nuanmb_v12;
#[path = "nuanmb_v12_encode.rs"]
pub mod nuanmb_v12_encode;
use anim_create_v12::{create_anim_v12, create_anim_v12_uncompressed};

/// Data associated with an [Anim] file.
/// Supported versions are 2.0 and 2.1.
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
    /// Converts the AnimData to an uncompressed Anim v1.2 format.
    /// 
    /// This method creates a v1.2 animation file using only constant and raw stream formats,
    /// never using compressed formats like 0x3409 or 0x4409. This is useful for maximum
    /// simplicity and compatibility when converting from other animation formats.
    /// 
    /// # Encoding Strategy
    /// - Constant tracks (all frames identical): Uses constant formats (0x3003, 0x4003, etc.)
    /// - Multi-frame tracks: Uses raw stream formats (0x3400, 0x4300, etc.)
    /// - Quaternions are automatically normalized before writing
    /// 
    /// # Errors
    /// Returns an error if:
    /// - The major/minor version is not 1.2
    /// - The final_frame_index is negative
    /// - There are issues writing the buffer data
    pub fn to_anim_v12_uncompressed(&self) -> Result<Anim, error::Error> {
        if self.major_version != 1 || self.minor_version != 2 {
            return Err(error::Error::UnsupportedVersion {
                major_version: self.major_version,
                minor_version: self.minor_version,
            });
        }
        create_anim_v12_uncompressed(self)
    }

    /// Writes the AnimData to a file in uncompressed Anim v1.2 format.
    ///
    /// This is a convenience method that combines `to_anim_v12_uncompressed()` with
    /// writing to a file. See `to_anim_v12_uncompressed()` for details on the encoding strategy.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The major/minor version is not 1.2
    /// - The final_frame_index is negative
    /// - There are issues writing the buffer data
    /// - File I/O fails
    pub fn write_to_file_v12_uncompressed<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), error::Error> {
        let anim = self.to_anim_v12_uncompressed()?;
        anim.write_to_file(path).map_err(Into::into)
    }

    /// Converts the AnimData to a compressed Anim v1.2 format compatible with EXVS2.
    ///
    /// This method creates a v1.2 animation file using compressed formats that match
    /// the game's expectations for playback, particularly 0x3409 for Vector3 data.
    ///
    /// # Encoding Strategy
    /// - Single-frame tracks: Uses constant formats (0x3003, 0x4003, etc.)
    /// - Multi-frame tracks: Uses compressed formats (0x3409 for Vector3, 0x4409 for quaternions)
    /// - Quaternions are automatically normalized before writing
    ///
    /// # Errors
    /// Returns an error if:
    /// - The major/minor version is not 1.2
    /// - The final_frame_index is negative
    /// - There are issues writing the buffer data
    pub fn to_anim_v12_compressed(&self) -> Result<Anim, error::Error> {
        if self.major_version != 1 || self.minor_version != 2 {
            return Err(error::Error::UnsupportedVersion {
                major_version: self.major_version,
                minor_version: self.minor_version,
            });
        }
        create_anim_v12(self)
    }

    /// Writes the AnimData to a file in compressed Anim v1.2 format compatible with EXVS2.
    ///
    /// This is a convenience method that combines `to_anim_v12_compressed()` with
    /// writing to a file. See `to_anim_v12_compressed()` for details on the encoding strategy.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The major/minor version is not 1.2
    /// - The final_frame_index is negative
    /// - There are issues writing the buffer data
    /// - File I/O fails
    pub fn write_to_file_v12_compressed<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), error::Error> {
        let anim = self.to_anim_v12_compressed()?;
        anim.write_to_file(path).map_err(Into::into)
    }
}

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
                    final_frame_index, ..
                } => *final_frame_index,
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

impl TryFrom<AnimData> for Anim {
    type Error = error::Error;

    fn try_from(data: AnimData) -> Result<Self, Self::Error> {
        create_anim(&data)
    }
}

impl TryFrom<&AnimData> for Anim {
    type Error = error::Error;

    fn try_from(data: &AnimData) -> Result<Self, Self::Error> {
        create_anim(data)
    }
}

pub mod error {
    use super::*;
    use thiserror::Error;

    /// Errors while creating an [Anim] from [AnimData].
    #[derive(Debug, Error)]
    pub enum Error {
        /// Creating an [Anim] file for the given version is not supported.
        #[error(
            "creating a version {}.{} anim is not supported",
            major_version,
            minor_version
        )]
        UnsupportedVersion {
            major_version: u16,
            minor_version: u16,
        },

        /// The final frame index is negative or smaller than the
        // index of the final frame in the longest track.
        #[error(
            "final frame index {} must be non negative and at least as 
             large as the index of the final frame in the longest track",
            final_frame_index
        )]
        InvalidFinalFrameIndex { final_frame_index: f32 },

        /// An error occurred while writing data to a buffer.
        #[error(transparent)]
        Io(#[from] std::io::Error),

        /// An error occurred while reading data from a buffer.
        #[error(transparent)]
        BinRead(#[from] binrw::error::Error),

        /// An error occurred while reading compressed data from a buffer.
        #[error(transparent)]
        BitError(#[from] bitutils::BitReadError),

        #[error(
            "compressed header bits per entry of {} does not match expected value of {}",
            actual,
            expected
        )]
        UnexpectedBitCount { expected: usize, actual: usize },

        #[error(
            "track data range {0}..{0}+{1} is out of range for a buffer of size {2}",
            start,
            size,
            buffer_size
        )]
        InvalidTrackDataRange {
            start: usize,
            size: usize,
            buffer_size: usize,
        },

        /// The buffer index is not valid for a version 1.2 anim file.
        #[error(
            "buffer index {} is out of range for a buffer collection of size {}",
            buffer_index,
            buffer_count
        )]
        BufferIndexOutOfRange {
            buffer_index: usize,
            buffer_count: usize,
        },

        #[error("the provided animation data is malformed or incomplete")]
        InvalidData,

        /// An error occurred while reading the compressed header for version 2.0 or later.
        #[error("the track data compression header is malformed and cannot be read")]
        MalformedCompressionHeader,
    }
}

enum AnimVersion {
    Version20,
    Version21,
}

// TODO: Test this for a small example?
fn create_anim(data: &AnimData) -> Result<Anim, error::Error> {
    let version = match (data.major_version, data.minor_version) {
        // Default to uncompressed format for v1.2.
        // Use `AnimData::to_anim_v12_compressed()` for EXVS2 compatible compression.
        (1, 2) => return create_anim_v12_uncompressed(data),
        (2, 0) => Ok(AnimVersion::Version20),
        (2, 1) => Ok(AnimVersion::Version21),
        _ => Err(error::Error::UnsupportedVersion {
            major_version: data.major_version,
            minor_version: data.minor_version,
        }),
    }?;

    let mut buffer = Cursor::new(Vec::new());

    let animations = data
        .groups
        .iter()
        .map(|g| create_anim_group(g, &mut buffer))
        .collect::<Result<Vec<_>, _>>()?;

    let max_frame_count = animations
        .iter()
        .filter_map(|a| {
            a.nodes
                .elements
                .iter()
                .filter_map(|n| n.tracks.elements.iter().map(|t| t.frame_count).max())
                .max()
        })
        .max()
        .unwrap_or(0);

    // Make sure the final frame index is at least as large as the final frame of the longest animation.
    let final_frame_index = if data.final_frame_index >= 0.0
        && data.final_frame_index >= max_frame_count as f32 - 1.0
    {
        Ok(data.final_frame_index)
    } else {
        Err(error::Error::InvalidFinalFrameIndex {
            final_frame_index: data.final_frame_index,
        })
    }?;

    match version {
        AnimVersion::Version20 => Ok(Anim::V20 {
            final_frame_index,
            unk1: 1,
            unk2: 3,
            name: "".into(), // TODO: this is usually based on file name?
            groups: animations.into(),
            buffer: buffer.into_inner().into(),
        }),
        AnimVersion::Version21 => Ok(Anim::V21 {
            final_frame_index,
            unk1: 1,
            unk2: 3,
            name: "".into(), // TODO: this is usually based on file name?
            groups: animations.into(),
            buffer: buffer.into_inner().into(),
            // TODO: Research how to rebuild the extra header data.
            unk_data: UnkData {
                unk1: SsbhArray::new(),
                unk2: SsbhArray::new(),
            },
        }),
    }
}

fn create_anim_group(g: &GroupData, buffer: &mut Cursor<Vec<u8>>) -> Result<Group, error::Error> {
    Ok(Group {
        group_type: g.group_type,
        nodes: g
            .nodes
            .iter()
            .map(|n| create_anim_node(n, buffer))
            .collect::<Result<Vec<_>, _>>()?
            .into(),
    })
}

fn create_anim_node(n: &NodeData, buffer: &mut Cursor<Vec<u8>>) -> Result<Node, error::Error> {
    Ok(Node {
        name: n.name.as_str().into(), // TODO: Make a convenience method for this?
        tracks: n
            .tracks
            .iter()
            .map(|t| create_anim_track_v2(buffer, t))
            .collect::<Result<Vec<_>, _>>()?
            .into(),
    })
}

fn create_anim_track_v2(
    buffer: &mut Cursor<Vec<u8>>,
    t: &TrackData,
) -> Result<TrackV2, error::Error> {
    let compression_type = infer_optimal_compression_type(&t.values);

    // The current stream position matches the offsets used for Smash Ultimate's anim files.
    // This assumes we traverse the hierarchy (group -> node -> track) in DFS order.
    let pos_before = buffer.stream_position()?;

    // Pointers for compressed data are relative to the start of the track's data.
    // This requires using a second writer due to how SsbhWrite is implemented.
    let mut track_data = Cursor::new(Vec::new());

    // TODO: Add tests for preserving scale compensation?.
    t.values
        .write(&mut track_data, compression_type, t.compensate_scale)?;

    buffer.write_all(&track_data.into_inner())?;
    let pos_after = buffer.stream_position()?;

    Ok(TrackV2 {
        name: t.name.as_str().into(),
        flags: TrackFlags {
            track_type: t.values.track_type(),
            compression_type,
        },
        frame_count: t.values.len() as u32,
        transform_flags: t.transform_flags.into(),
        data_offset: pos_before as u32,
        data_size: pos_after - pos_before,
    })
}

fn infer_optimal_compression_type(values: &TrackValues) -> CompressionType {
    match (values, values.len()) {
        // Single frame animations use a special compression type.
        (TrackValues::Transform(_), 0..=1) => CompressionType::ConstTransform,
        (_, 0..=1) => CompressionType::Constant,
        _ => {
            // The compressed header adds some overhead, so we need to also check frame count.
            // Once there are enough elements to exceed the header size, compression starts to save space.

            // TODO: Is integer division correct here?
            let uncompressed_frames_per_header =
                values.compressed_overhead_in_bytes() / values.data_size_in_bytes();

            // Some tracks overlap the default data with the compression to save space.
            // This calculation assumes we aren't performing that optimization.
            if values.len() > uncompressed_frames_per_header as usize + 1 {
                CompressionType::Compressed
            } else {
                CompressionType::Direct
            }
        }
    }
}

// TODO: Test conversions from anim?
fn read_anim_groups(anim: &Anim) -> Result<Vec<GroupData>, error::Error> {
    match anim {
        // TODO: Create fake groups for version 1.0?
        ssbh_lib::prelude::Anim::V12 {
            tracks, buffers, final_frame_index, ..
        } => {
            // For version 1.2, use the animation's final_frame_index to determine frame count
            let frame_count = (*final_frame_index as usize).saturating_add(1);
            read_groups_v12(&tracks.elements, &buffers.elements, frame_count)
        }
        ssbh_lib::formats::anim::Anim::V20 { groups, buffer, .. } => {
            read_groups_v20(&groups.elements, &buffer.elements)
        }
        ssbh_lib::formats::anim::Anim::V21 { groups, buffer, .. } => {
            read_groups_v20(&groups.elements, &buffer.elements)
        }
    }
}

fn group_type_v12(track_type: TrackTypeV1) -> GroupType {
    match track_type {
        TrackTypeV1::Transform => GroupType::Transform,
        TrackTypeV1::UvTransform => GroupType::Material,
        TrackTypeV1::Visibility => GroupType::Visibility,
    }
}

fn read_groups_v12(
    tracks: &[ssbh_lib::formats::anim::TrackV1],
    buffers: &[ssbh_lib::SsbhByteBuffer],
    animation_frame_count: usize,
) -> Result<Vec<GroupData>, error::Error> {
    // Group by the track type.
    let mut tracks_by_type = HashMap::new();

    // Node names like bones names are set at the track level for anim 1.2.
    // Save the track name to use for the nodes later.
    for track in tracks {
        let group_type = group_type_v12(track.track_type);
        let track_data = create_track_data_v12(track, buffers, animation_frame_count)?;
        tracks_by_type
            .entry(group_type)
            .or_insert(Vec::new())
            .push((track.name.to_string_lossy(), track_data));
    }

    // Use the grouping conventions for version 2.0+ anims.
    // TODO: Will this preserve data when saving back to 1.2?
    let groups = tracks_by_type
        .into_iter()
        .map(|(group_type, tracks)| GroupData {
            group_type,
            nodes: tracks
                .into_iter()
                .map(|(name, track)| NodeData {
                    name,
                    tracks: vec![track],
                })
                .collect(),
        })
        .collect();

    Ok(groups)
}

// Improved version 1.2 track data creation that properly merges properties
fn create_track_data_v12(
    track: &ssbh_lib::formats::anim::TrackV1,
    buffers: &[ssbh_lib::SsbhByteBuffer],
    animation_frame_count: usize,
) -> Result<TrackData, error::Error> {
    let mut compensate_scale = false;
    let transform_flags = TransformFlags::default();
    
    // Collect parsed property data
    struct PropertyData {
        scales: Vec<Vector3>,
        rotations: Vec<Vector4>,
        translations: Vec<Vector3>,
        visibilities: Vec<bool>,
        uv_transforms: Vec<UvTransform>,
    }
    
    let mut property_data = PropertyData {
        scales: Vec::new(),
        rotations: Vec::new(),
        translations: Vec::new(),
        visibilities: Vec::new(),
        uv_transforms: Vec::new(),
    };
    
    // First pass: parse all properties and determine frame count
    for property in &track.properties.elements {
        let property_name = property.name.to_string_lossy();
        let data = buffers.get(property.buffer_index as usize).ok_or(
            error::Error::BufferIndexOutOfRange {
                buffer_index: property.buffer_index as usize,
                buffer_count: buffers.len(),
            },
        )?;

        let mut reader = Cursor::new(&data.elements);
        let header: u32 = reader.read_le()?;

        match &property_name as &str {
            "CompensateScale" => {
                match header {
                    0x1013 => {
                        let value: u16 = reader.read_le()?;
                        compensate_scale = value != 0;
                    }
                    0x1003 => {
                        let value: f32 = reader.read_le()?;
                        compensate_scale = value != 0.0;
                    }
                    _ => {}
                }
            }
            "Scale" => {
                match header {
                    0x3003 => {
                        // Single scale value
                        let scale = reader.read_le::<Vector3>()?;
                        property_data.scales.push(scale);
                    }
                    0x3200 => {
                        let frames = nuanmb_v12::decode_scale_3200(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    0x3208 => {
                        let frames = nuanmb_v12::decode_scale_3208(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    0x3209 => {
                        let frames = nuanmb_v12::decode_scale_3209(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    0x3300 => {
                        let frames = nuanmb_v12::decode_scale_3300(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    0x3308 => {
                        let frames = nuanmb_v12::decode_scale_3308(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    0x3309 => {
                        let frames = nuanmb_v12::decode_scale_3309(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    0x3400 => {
                        let frames = nuanmb_v12::decode_scale_3400(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    0x3408 => {
                        let frames = nuanmb_v12::decode_scale_3408(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    0x3409 => {
                        let frames = nuanmb_v12::decode_scale_3409(&data.elements)?;
                        property_data.scales.extend(frames);
                    }
                    _ => {
                        // For complex scale formats, use default
                        property_data.scales.push(Vector3 { x: 1.0, y: 1.0, z: 1.0 });
                    }
                }
            }
            "Rotate" => {
                match header {
                    0x3003 => {
                        // Single Vector3 (euler angles, convert to quaternion) - uncompressed
                        let euler: Vector3 = reader.read_le()?;
                        let rotation = euler_to_quaternion(euler);
                        property_data.rotations.push(rotation);
                    }
                    0x4003 => {
                        // Single Vector4 (quaternion rotation) - uncompressed
                        let rotation = reader.read_le::<Vector4>()?;
                        property_data.rotations.push(rotation);
                    }
                    0x4200 => {
                        let frames = nuanmb_v12::decode_rotate_4200(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    0x4208 => {
                        let frames = nuanmb_v12::decode_rotate_4208(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    0x4209 => {
                        let frames = nuanmb_v12::decode_rotate_4209(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    0x4300 => {
                        let frames = nuanmb_v12::decode_rotate_4300(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    0x4308 => {
                        let frames = nuanmb_v12::decode_rotate_4308(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    0x4309 => {
                        let frames = nuanmb_v12::decode_rotate_4309(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    0x4400 => {
                        let frames = nuanmb_v12::decode_rotate_4400(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    0x4408 => {
                        let frames = nuanmb_v12::decode_rotate_4408(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    0x4409 => {
                        let frames = nuanmb_v12::decode_rotate_4409(&data.elements)?;
                        property_data.rotations.extend(frames);
                    }
                    _ => {
                        // Unknown format, use default identity rotation
                        property_data.rotations.push(Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 });
                    }
                }
            }
            "Translate" => {
                match header {
                    0x3003 => {
                        let translation = reader.read_le::<Vector3>()?;
                        property_data.translations.push(translation);
                    }
                    0x3200 => {
                        let frames = nuanmb_v12::decode_translate_3200(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    0x3208 => {
                        let frames = nuanmb_v12::decode_translate_3208(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    0x3209 => {
                        let frames = nuanmb_v12::decode_translate_3209(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    0x3300 => {
                        let frames = nuanmb_v12::decode_translate_3300(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    0x3308 => {
                        let frames = nuanmb_v12::decode_translate_3308(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    0x3309 => {
                        let frames = nuanmb_v12::decode_translate_3309(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    0x3400 => {
                        let frames = nuanmb_v12::decode_translate_3400(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    0x3408 => {
                        let frames = nuanmb_v12::decode_translate_3408(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    0x3409 => {
                        let frames = nuanmb_v12::decode_translate_3409(&data.elements)?;
                        property_data.translations.extend(frames);
                    }
                    _ => {
                        // Unknown format
                        property_data.translations.push(Vector3 { x: 0.0, y: 0.0, z: 0.0 });
                    }
                }
            }
            "Visibility" => {
                match header {
                    0x1013 => {
                        let value: u16 = reader.read_le()?;
                        property_data.visibilities.push(value != 0);
                    }
                    _ => {
                        property_data.visibilities.push(true);
                    }
                }
            }
            _ => {
                // Handle other unknown properties
            }
        }
    }
    
    // Second pass: create properly merged animation data
    let values = match track.track_type {
        TrackTypeV1::Transform => {
            let mut transforms = Vec::new();

            for frame_idx in 0..animation_frame_count {
                let scale = property_data.scales.get(frame_idx)
                    .copied()
                    .unwrap_or_else(|| property_data.scales.last().copied().unwrap_or(Vector3 { x: 1.0, y: 1.0, z: 1.0 }));
                    
                let rotation = property_data.rotations.get(frame_idx)
                    .copied()
                    .unwrap_or_else(|| property_data.rotations.last().copied().unwrap_or(Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 }));
                    
                let translation = property_data.translations.get(frame_idx)
                    .copied()
                    .unwrap_or_else(|| property_data.translations.last().copied().unwrap_or(Vector3 { x: 0.0, y: 0.0, z: 0.0 }));
                
                transforms.push(Transform {
                    scale,
                    rotation,
                    translation,
                });
            }
            
            // If no frames were generated, create a default identity transform
            if transforms.is_empty() {
                transforms.push(Transform {
                    scale: Vector3 { x: 1.0, y: 1.0, z: 1.0 },
                    rotation: Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
                    translation: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
                });
            }
            
            TrackValues::Transform(transforms)
        }
        TrackTypeV1::Visibility => {
            if property_data.visibilities.is_empty() {
                TrackValues::Boolean(vec![true])
            } else {
                TrackValues::Boolean(property_data.visibilities)
            }
        }
        TrackTypeV1::UvTransform => {
            if property_data.uv_transforms.is_empty() {
                TrackValues::UvTransform(vec![UvTransform {
                    scale_u: 1.0,
                    scale_v: 1.0,
                    rotation: 0.0,
                    translate_u: 0.0,
                    translate_v: 0.0,
                }])
            } else {
                TrackValues::UvTransform(property_data.uv_transforms)
            }
        }
    };

    Ok(TrackData {
        name: match track.track_type {
            TrackTypeV1::Transform => "Transform".to_owned(),
            TrackTypeV1::Visibility => "Visibility".to_owned(),
            TrackTypeV1::UvTransform => "UvTransform".to_owned(),
        },
        compensate_scale,
        values,
        transform_flags,
    })
}

// Helper function to convert euler angles to quaternion
fn euler_to_quaternion(euler: Vector3) -> Vector4 {
    let (sx, cx) = (euler.x * 0.5).sin_cos();
    let (sy, cy) = (euler.y * 0.5).sin_cos();
    let (sz, cz) = (euler.z * 0.5).sin_cos();

    Vector4 {
        x: sx * cy * cz - cx * sy * sz,
        y: cx * sy * cz + sx * cy * sz,
        z: cx * cy * sz - sx * sy * cz,
        w: cx * cy * cz + sx * sy * sz,
    }
}

fn read_groups_v20(
    anim_groups: &[ssbh_lib::formats::anim::Group],
    anim_buffer: &[u8],
) -> Result<Vec<GroupData>, error::Error> {
    let mut groups = Vec::new();

    for anim_group in anim_groups {
        let mut nodes = Vec::new();

        for anim_node in &anim_group.nodes.elements {
            let mut tracks = Vec::new();
            for anim_track in &anim_node.tracks.elements {
                // Find and read the track data.
                let track = create_track_data_v20(anim_track, anim_buffer)?;
                tracks.push(track);
            }

            let node = NodeData {
                name: anim_node.name.to_string_lossy(),
                tracks,
            };
            nodes.push(node);
        }

        let group = GroupData {
            group_type: anim_group.group_type,
            nodes,
        };
        groups.push(group);
    }

    Ok(groups)
}

fn create_track_data_v20(
    track: &ssbh_lib::formats::anim::TrackV2,
    buffer: &[u8],
) -> Result<TrackData, error::Error> {
    let start = track.data_offset as usize;
    let end =
        start
            .checked_add(track.data_size as usize)
            .ok_or(error::Error::InvalidTrackDataRange {
                start: track.data_offset as usize,
                size: track.data_size as usize,
                buffer_size: buffer.len(),
            })?;
    let buffer = buffer
        .get(start..end)
        .ok_or(error::Error::InvalidTrackDataRange {
            start: track.data_offset as usize,
            size: track.data_size as usize,
            buffer_size: buffer.len(),
        })?;

    let (values, compensate_scale) =
        read_track_values(buffer, track.flags, track.frame_count as usize)?;

    // The compensate scale override is included in scale options instead.
    Ok(TrackData {
        name: track.name.to_string_lossy(),
        values,
        compensate_scale,
        transform_flags: track.transform_flags.into(),
    })
}

/// Data associated with a [Group].
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Clone)]
pub struct GroupData {
    /// The usage type for all the [NodeData] in [nodes](#structfield.nodes)
    pub group_type: GroupType,
    pub nodes: Vec<NodeData>,
}

/// Data associated with a [Node].
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, PartialEq, Clone)]
pub struct NodeData {
    pub name: String,
    pub tracks: Vec<TrackData>,
}

/// The data associated with a [TrackV2].
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
        Self::new()
            .with_override_translation(f.override_translation)
            .with_override_rotation(f.override_rotation)
            .with_override_scale(f.override_scale)
            .with_override_compensate_scale(f.override_compensate_scale)
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
    /// XYZ scale
    pub scale: Vector3,
    /// An XYZW unit quaternion where XYZ represent the axis component
    /// and w represents the angle component.
    pub rotation: Vector4,
    /// XYZ translation
    pub translation: Vector3,
}

impl Transform {
    /// An identity transformation representing no scale, rotation, or translation.
    pub const IDENTITY: Transform = Transform {
        scale: Vector3 {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
        rotation: Vector4 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        },
        translation: Vector3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
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
    Vector4(Vec<Vector4>),
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

// TODO: Organize this in compression.rs similar to version 2.0+
// Version 1.2 compression structures based on GitHub research

/// Header for version 1.2 compressed Vector3 data (0x0934, 0x3409)
#[allow(dead_code)]
#[derive(Debug, BinRead)]
struct V12Vector3CompressedHeader {
    /// Always 0x0934 or 0x3409
    header: u32,
    /// Number of frames in the animation
    frame_count: u32,
    /// Unknown float 1, typically 1.0
    unk1: f32,
    /// Unknown float 2, varies
    unk2: f32,
    /// Flags, typically 2
    flags: u16,
    /// Bits per entry for decompression
    bits_per_entry: u16,
    /// Default values: first, middle, and last keyframes
    default_values: [Vector3; 3],
}

/// Header for version 1.2 compressed Vector4 data (0x0944, 0x4409)
#[allow(dead_code)]
#[derive(Debug, BinRead)]
struct V12Vector4CompressedHeader {
    /// Always 0x0944 or 0x4409
    header: u32,
    /// Number of frames in the animation
    frame_count: u32,
    /// Unknown float 1, typically 1.0
    unk1: f32,
    /// Unknown float 2, varies
    unk2: f32,
    /// Flags, typically 2
    flags: u16,
    /// Bits per entry for decompression
    bits_per_entry: u16,
    /// Default values: first, middle, and last keyframes
    default_values: [Vector4; 3],
}

/// Header for version 1.2 compressed Vector3 data with frame indices (0x4308)
#[allow(dead_code)]
#[derive(Debug, BinRead)]
struct V12Vector3IndexedCompressedHeader {
    /// Always 0x4308
    header: u32,
    /// Number of frames in the animation
    frame_count: u32,
    /// Unknown float 1, typically 1.0
    unk1: f32,
    /// Frame indices (one byte per frame)
    #[br(count = frame_count, align_after = 4)]
    frame_indices: Vec<u8>,
    /// Default values: first, middle, and last keyframes
    default_values: [Vector3; 3],
    /// Additional 9 floats (purpose unclear)
    additional_floats: [f32; 9],
    /// Bits per entry for decompression (calculated)
    bits_per_entry: u16,
}

// Legacy test structures (keeping for compatibility)
#[allow(dead_code)]
#[derive(Debug, BinRead)]
struct V12Test1 {
    unk0: u32, // frame count?
    unk1: f32,
    unk2: f32,
    unk3: u16, // flags?
    unk4: u16,
    unk5: [Vector3; 3],
    // TODO: Compressed data?
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
struct V12Test2 {
    unk0: u32, // frame count?
    unk1: f32,
    unk2: f32,
    unk3: u16, // flags?
    unk4: u16,
    unk5: [Vector4; 3],
    // TODO: Compressed data?
}

#[allow(dead_code)]
#[derive(Debug, BinRead)]
struct V12Test3 {
    frame_count: u32,
    unk1: f32,
    #[br(count = frame_count, align_after = 4)] // align to float boundary
    unk2: Vec<u8>, // TODO: key frames?
    unk3: [Vector3; 3],
    // TODO: Compressed data?
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Test the conversions more thoroughly.

    #[test]
    fn create_empty_anim_v_2_0() {
        let anim = create_anim(&AnimData {
            major_version: 2,
            minor_version: 0,
            final_frame_index: 1.5,
            groups: Vec::new(),
        })
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
        let anim = create_anim(&AnimData {
            major_version: 2,
            minor_version: 1,
            final_frame_index: 2.5,
            groups: Vec::new(),
        })
        .unwrap();

        assert!(matches!(anim, Anim::V21 {
            final_frame_index, 
            ..
        } if final_frame_index == 2.5));
    }

    #[test]
    fn create_anim_negative_frame_index() {
        let result = create_anim(&AnimData {
            major_version: 2,
            minor_version: 1,
            final_frame_index: -1.0,
            groups: Vec::new(),
        });

        assert!(matches!(
            result,
            Err(error::Error::InvalidFinalFrameIndex {
                final_frame_index
            }) if final_frame_index == -1.0
        ));
    }

    #[test]
    fn create_anim_insufficient_frame_index() {
        let result = create_anim(&AnimData {
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
        });

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
        let anim = create_anim(&AnimData {
            major_version: 2,
            minor_version: 1,
            final_frame_index: 0.0,
            groups: Vec::new(),
        })
        .unwrap();

        assert!(matches!(anim, Anim::V21 {
            final_frame_index,
            ..
        } if final_frame_index == 0.0));
    }

    #[test]
    fn create_empty_anim_invalid_version() {
        let result = create_anim(&AnimData {
            major_version: 3,
            minor_version: 0,
            final_frame_index: 0.0,
            groups: Vec::new(),
        });

        assert!(matches!(
            result,
            Err(error::Error::UnsupportedVersion {
                major_version: 3,
                minor_version: 0
            })
        ));
    }

    #[test]
    fn create_node_no_tracks() {
        let node = NodeData {
            name: "empty".to_string(),
            tracks: Vec::new(),
        };

        let mut buffer = Cursor::new(Vec::new());

        let anim_node = create_anim_node(&node, &mut buffer).unwrap();
        assert_eq!("empty", anim_node.name.to_str().unwrap());
        assert!(anim_node.tracks.elements.is_empty());
    }

    #[test]
    fn create_node_multiple_tracks() {
        let node = NodeData {
            name: "empty".to_string(),
            tracks: vec![
                TrackData {
                    name: "t1".to_string(),
                    values: TrackValues::Float(vec![1.0, 2.0, 3.0]),
                    compensate_scale: false,
                    transform_flags: TransformFlags::default(),
                },
                TrackData {
                    name: "t2".to_string(),
                    values: TrackValues::PatternIndex(vec![4, 5]),
                    compensate_scale: false,
                    transform_flags: TransformFlags::default(),
                },
            ],
        };

        let mut buffer = Cursor::new(Vec::new());

        let anim_node = create_anim_node(&node, &mut buffer).unwrap();
        assert_eq!("empty", anim_node.name.to_str().unwrap());
        assert_eq!(2, anim_node.tracks.elements.len());

        let t1 = &anim_node.tracks.elements[0];
        assert_eq!("t1", t1.name.to_str().unwrap());
        assert_eq!(
            TrackFlags {
                track_type: TrackTypeV2::Float,
                compression_type: CompressionType::Direct
            },
            t1.flags
        );
        assert_eq!(3, t1.frame_count);
        assert_eq!(0, t1.data_offset);
        assert_eq!(12, t1.data_size);

        let t2 = &anim_node.tracks.elements[1];
        assert_eq!("t2", t2.name.to_str().unwrap());
        assert_eq!(
            TrackFlags {
                track_type: TrackTypeV2::PatternIndex,
                compression_type: CompressionType::Direct
            },
            t2.flags
        );
        assert_eq!(2, t2.frame_count);
        assert_eq!(12, t2.data_offset);
        assert_eq!(8, t2.data_size);
    }

    #[test]
    fn compression_type_empty() {
        assert_eq!(
            CompressionType::ConstTransform,
            infer_optimal_compression_type(&TrackValues::Transform(Vec::new()))
        );
        assert_eq!(
            CompressionType::Constant,
            infer_optimal_compression_type(&TrackValues::UvTransform(Vec::new()))
        );
        assert_eq!(
            CompressionType::Constant,
            infer_optimal_compression_type(&TrackValues::Float(Vec::new()))
        );
        assert_eq!(
            CompressionType::Constant,
            infer_optimal_compression_type(&TrackValues::PatternIndex(Vec::new()))
        );
        assert_eq!(
            CompressionType::Constant,
            infer_optimal_compression_type(&TrackValues::Boolean(Vec::new()))
        );
        assert_eq!(
            CompressionType::Constant,
            infer_optimal_compression_type(&TrackValues::Vector4(Vec::new()))
        );
    }

    #[test]
    fn compression_type_boolean_multiple_frames() {
        // The compression adds 33 bytes of overhead.
        // The uncompressed representation for a bool is 1 byte.
        // We need more than (33 / 1 + 1) frames for compression to save space.
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::Boolean(vec![true; 8]))
        );
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::Boolean(vec![true; 34]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::Boolean(vec![true; 35]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::Boolean(vec![true; 100]))
        );
    }

    #[test]
    fn compression_type_float_multiple_frames() {
        // The compression adds 36 bytes of overhead.
        // The uncompressed representation for a float is 4 bytes.
        // We need more than 10 (36 / 4 + 1) frames for compression to save space.
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::Float(vec![0.0; 8]))
        );
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::Float(vec![0.0; 10]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::Float(vec![0.0; 11]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::Float(vec![0.0; 100]))
        );
    }

    #[test]
    fn compression_type_pattern_index_multiple_frames() {
        // The compression adds 36 bytes of overhead.
        // The uncompressed representation for a float is 4 bytes.
        // We need more than 10 (36 / 4 + 1) frames for compression to save space.
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::PatternIndex(vec![0; 8]))
        );
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::PatternIndex(vec![0; 10]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::PatternIndex(vec![0; 11]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::PatternIndex(vec![0; 100]))
        );
    }

    #[test]
    fn compression_type_uv_transform_multiple_frames() {
        // The compression adds 116 bytes of overhead.
        // The uncompressed representation for a UV transform is 20 bytes.
        // We need more than 6.8 (116 / 20 + 1) frames for compression to save space.
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::UvTransform(vec![
                UvTransform::default();
                3
            ]))
        );
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::UvTransform(vec![
                UvTransform::default();
                6
            ]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::UvTransform(vec![
                UvTransform::default();
                7
            ]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::UvTransform(vec![
                UvTransform::default();
                100
            ]))
        );
    }

    #[test]
    fn compression_type_vector4_multiple_frames() {
        // The compression adds 96 bytes of overhead.
        // The uncompressed representation for a UV transform is 20 bytes.
        // We need more than 7 (96 / 16 + 1) frames for compression to save space.
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::Vector4(vec![Vector4::default(); 3]))
        );
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::Vector4(vec![Vector4::default(); 7]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::Vector4(vec![Vector4::default(); 8]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::Vector4(vec![Vector4::default(); 100]))
        );
    }

    #[test]
    fn compression_type_transform_multiple_frames() {
        // The compression adds 204 bytes of overhead.
        // The uncompressed representation for a transform is 44 bytes.
        // We need more than 5.63 (204 / 44 + 1) frames for compression to save space.
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::Transform(vec![Transform::default(); 3]))
        );
        assert_eq!(
            CompressionType::Direct,
            infer_optimal_compression_type(&TrackValues::Transform(vec![Transform::default(); 5]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::Transform(vec![Transform::default(); 6]))
        );
        assert_eq!(
            CompressionType::Compressed,
            infer_optimal_compression_type(&TrackValues::Transform(vec![
                Transform::default();
                100
            ]))
        );
    }

    #[test]
    fn read_v20_track_invalid_offset() {
        let result = create_track_data_v20(
            &TrackV2 {
                name: "abc".into(),
                flags: TrackFlags {
                    track_type: TrackTypeV2::Transform,
                    compression_type: CompressionType::Compressed,
                },
                frame_count: 2,
                transform_flags: AnimTransformFlags::new(),
                data_offset: 5,
                data_size: 1,
            },
            &[0u8; 4],
        );

        assert!(matches!(
            result,
            Err(error::Error::InvalidTrackDataRange {
                start: 5,
                size: 1,
                buffer_size: 4
            })
        ));
    }

    #[test]
    fn read_v20_track_offset_overflow() {
        let result = create_track_data_v20(
            &TrackV2 {
                name: "abc".into(),
                flags: TrackFlags {
                    track_type: TrackTypeV2::Transform,
                    compression_type: CompressionType::Compressed,
                },
                frame_count: 2,
                transform_flags: AnimTransformFlags::new(),
                data_offset: u32::MAX,
                data_size: 1,
            },
            &[0u8; 4],
        );

        assert!(matches!(
            result,
            Err(error::Error::InvalidTrackDataRange {
                start: 4294967295,
                size: 1,
                buffer_size: 4
            })
        ));
    }

    #[test]
    fn read_v20_track_invalid_size() {
        let result = create_track_data_v20(
            &TrackV2 {
                name: "abc".into(),
                flags: TrackFlags {
                    track_type: TrackTypeV2::Transform,
                    compression_type: CompressionType::Compressed,
                },
                frame_count: 2,
                transform_flags: AnimTransformFlags::new(),
                data_offset: 0,
                data_size: 5,
            },
            &[0u8; 3],
        );

        assert!(matches!(
            result,
            Err(error::Error::InvalidTrackDataRange {
                start: 0,
                size: 5,
                buffer_size: 3
            })
        ));
    }
}

