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
//! Compressed animations use lossy compression for all data types except [TrackValues::Boolean].
//! When converting to [Anim], compression is enabled for a track if compression would save space.
//! This may produce differences with the original due to compression differences.
//! These errors are small in practice but may cause gameplay differences such as online desyncs.
use binrw::io::{Cursor, Read, Seek, Write};
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

        /// An error occurred while reading the compressed header for version 2.0 or later.
        #[error("the track data compression header is malformed and cannot be read")]
        MalformedCompressionHeader,
    }
}

enum AnimVersion {
    Version20,
    Version21,
}

// Function to create version 1.2 animation from AnimData
fn create_anim_v12(data: &AnimData) -> Result<Anim, error::Error> {
    use ssbh_lib::formats::anim::{TrackV1, Property};
    use ssbh_lib::SsbhByteBuffer;
    
    let mut tracks = Vec::new();
    let mut buffers = Vec::new();
    
    // Convert each group back to tracks for version 1.2
    for group in &data.groups {
        for node in &group.nodes {
            for track in &node.tracks {
                // Determine track type based on group type and track name
                let track_type = match group.group_type {
                    GroupType::Transform => TrackTypeV1::Transform,
                    GroupType::Visibility => TrackTypeV1::Visibility,
                    GroupType::Material => TrackTypeV1::UvTransform,
                    _ => TrackTypeV1::Transform, // Default fallback
                };
                
                // Create properties based on track type and values
                let mut properties = Vec::new();
                
                match (&track.values, track_type) {
                    (TrackValues::Transform(transforms), TrackTypeV1::Transform) => {
                        if !transforms.is_empty() {
                            // Extract scale, rotation, and translation data
                            let scales: Vec<Vector3> = transforms.iter().map(|t| t.scale).collect();
                            let rotations: Vec<Vector4> = transforms.iter().map(|t| t.rotation).collect();
                            let translations: Vec<Vector3> = transforms.iter().map(|t| t.translation).collect();
                            
                            // Create Scale property
                            if scales.len() == 1 {
                                // Single frame scale
                                let mut scale_data = Vec::new();
                                scale_data.extend_from_slice(&0x3003u32.to_le_bytes());
                                scale_data.extend_from_slice(&scales[0].x.to_le_bytes());
                                scale_data.extend_from_slice(&scales[0].y.to_le_bytes());
                                scale_data.extend_from_slice(&scales[0].z.to_le_bytes());
                                
                                buffers.push(SsbhByteBuffer { elements: scale_data });
                                properties.push(Property {
                                    name: "Scale".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            } else {
                                // Multi-frame scale data - use compressed format
                                let scale_data = create_v12_compressed_vector3_data(&scales)?;
                                buffers.push(SsbhByteBuffer { elements: scale_data });
                                properties.push(Property {
                                    name: "Scale".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            }
                            
                            // Create Rotation property
                            if rotations.len() == 1 {
                                // Single frame rotation
                                let mut rotation_data = Vec::new();
                                rotation_data.extend_from_slice(&0x4003u32.to_le_bytes());
                                rotation_data.extend_from_slice(&rotations[0].x.to_le_bytes());
                                rotation_data.extend_from_slice(&rotations[0].y.to_le_bytes());
                                rotation_data.extend_from_slice(&rotations[0].z.to_le_bytes());
                                rotation_data.extend_from_slice(&rotations[0].w.to_le_bytes());
                                
                                buffers.push(SsbhByteBuffer { elements: rotation_data });
                                properties.push(Property {
                                    name: "Rotate".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            } else {
                                // Multi-frame rotation data - use compressed format
                                let rotation_data = create_v12_compressed_vector4_data(&rotations)?;
                                buffers.push(SsbhByteBuffer { elements: rotation_data });
                                properties.push(Property {
                                    name: "Rotate".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            }
                            
                            // Create Translation property
                            if translations.len() == 1 {
                                // Single frame translation
                                let mut translation_data = Vec::new();
                                translation_data.extend_from_slice(&0x3003u32.to_le_bytes());
                                translation_data.extend_from_slice(&translations[0].x.to_le_bytes());
                                translation_data.extend_from_slice(&translations[0].y.to_le_bytes());
                                translation_data.extend_from_slice(&translations[0].z.to_le_bytes());
                                
                                buffers.push(SsbhByteBuffer { elements: translation_data });
                                properties.push(Property {
                                    name: "Translate".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            } else {
                                // Multi-frame translation data - use compressed format
                                let translation_data = create_v12_compressed_vector3_data(&translations)?;
                                buffers.push(SsbhByteBuffer { elements: translation_data });
                                properties.push(Property {
                                    name: "Translate".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            }
                            
                            // CompensateScale property if needed
                            if track.compensate_scale {
                                let mut compensate_data = Vec::new();
                                compensate_data.extend_from_slice(&0x1003u32.to_le_bytes());
                                compensate_data.extend_from_slice(&1.0f32.to_le_bytes()); // true = 1.0
                                
                                buffers.push(SsbhByteBuffer { elements: compensate_data });
                                properties.push(Property {
                                    name: "CompensateScale".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            }
                        }
                    }
                    (TrackValues::Boolean(bools), TrackTypeV1::Visibility) => {
                        if !bools.is_empty() {
                            if bools.len() == 1 {
                                // Single frame visibility
                                let mut visibility_data = Vec::new();
                                visibility_data.extend_from_slice(&0x1013u32.to_le_bytes());
                                visibility_data.extend_from_slice(&(if bools[0] { 1u16 } else { 0u16 }).to_le_bytes());
                                
                                buffers.push(SsbhByteBuffer { elements: visibility_data });
                                properties.push(Property {
                                    name: "Visibility".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            } else {
                                // Multi-frame visibility data - create appropriate compressed format
                                let visibility_data = create_v12_compressed_bool_data(bools)?;
                                buffers.push(SsbhByteBuffer { elements: visibility_data });
                                properties.push(Property {
                                    name: "Visibility".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            }
                        }
                    }
                    (TrackValues::UvTransform(uv_transforms), TrackTypeV1::UvTransform) => {
                        if !uv_transforms.is_empty() {
                            if uv_transforms.len() == 1 {
                                // Single frame UV transform
                                let uv = &uv_transforms[0];
                                let mut uv_data = Vec::new();
                                uv_data.extend_from_slice(&0x5014u32.to_le_bytes());
                                uv_data.extend_from_slice(&uv.scale_u.to_le_bytes());
                                uv_data.extend_from_slice(&uv.scale_v.to_le_bytes());
                                uv_data.extend_from_slice(&uv.rotation.to_le_bytes());
                                uv_data.extend_from_slice(&uv.translate_u.to_le_bytes());
                                uv_data.extend_from_slice(&uv.translate_v.to_le_bytes());
                                
                                buffers.push(SsbhByteBuffer { elements: uv_data });
                                properties.push(Property {
                                    name: "UvTransform".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            } else {
                                // Multi-frame UV transform data - create appropriate compressed format
                                let uv_data = create_v12_compressed_uv_data(uv_transforms)?;
                                buffers.push(SsbhByteBuffer { elements: uv_data });
                                properties.push(Property {
                                    name: "UvTransform".into(),
                                    buffer_index: (buffers.len() - 1) as u64,
                                });
                            }
                        }
                    }
                    _ => {
                        // Create a default empty property for unsupported combinations
                        let mut default_data = Vec::new();
                        default_data.extend_from_slice(&0x0000u32.to_le_bytes());
                        
                        buffers.push(SsbhByteBuffer { elements: default_data });
                        properties.push(Property {
                            name: track.name.as_str().into(),
                            buffer_index: (buffers.len() - 1) as u64,
                        });
                    }
                }
                
                // Create the track
                tracks.push(TrackV1 {
                    name: node.name.as_str().into(),
                    track_type,
                    properties: properties.into(),
                });
            }
        }
    }
    
    Ok(Anim::V12 {
        name: "".into(), // Default empty name
        unk1: 0.0,       // Default unknown value
        final_frame_index: data.final_frame_index,
        unk2: 0.0,       // Default unknown value
        unk3: 0.0,       // Default unknown value
        tracks: tracks.into(),
        buffers: buffers.into(),
    })
}

// TODO: Test this for a small example?
fn create_anim(data: &AnimData) -> Result<Anim, error::Error> {
    let version = match (data.major_version, data.minor_version) {
        (1, 2) => return create_anim_v12(data),
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
                    _ => {
                        // For complex scale formats, use default
                        property_data.scales.push(Vector3 { x: 1.0, y: 1.0, z: 1.0 });
                    }
                }
            }
            "Rotate" => {
                match header {
                    0x3409 => {
                        // Version 1.2: Compressed Vector3-based rotation data (Euler angles)
                        // Based on GitHub discussion format
                        let _compressed_frame_count = reader.read_le::<u32>()? as usize;
                        let _unk1 = reader.read_le::<f32>()?;  // typically 0.0 or 1.0
                        let _unk2 = reader.read_le::<f32>()?;  // varies
                        let _flags = reader.read_le::<u16>()?; // compression flags
                        let bits_per_entry = reader.read_le::<u16>()? as usize;
                        
                        // Three default Vector3 values representing key frames (first, middle, last)
                        let default_values: [Vector3; 3] = reader.read_le()?;
                        
                        let remaining_data = data.elements.len() - (reader.position() as usize);
                        if remaining_data > 0 && bits_per_entry > 0 {
                            // Read compressed euler angle frames
                            let compressed_frames = read_v12_compressed_vector3_data(
                                &mut reader, _compressed_frame_count, &default_values, bits_per_entry
                            )?;
                            for rotation_euler in compressed_frames {
                                let rotation = euler_to_quaternion(rotation_euler);
                                property_data.rotations.push(rotation);
                            }
                        } else {
                            // No compressed data, use default value
                            let default_euler = default_values[0];
                            let rotation = euler_to_quaternion(default_euler);
                            property_data.rotations.push(rotation);
                        }
                    }
                    0x4308 => {
                        // Version 1.2: Compressed Vector3 data with special format (also for Euler rotation)
                        // This format includes frame indices and 9 additional floats
                        let _compressed_frame_count = reader.read_le::<u32>()? as usize;
                        let _unk1 = reader.read_le::<f32>()?;

                        // Read frame indices (one byte per frame)
                        let mut frame_indices = vec![0u8; _compressed_frame_count];
                        reader.read_exact(&mut frame_indices)?;
                        
                        // Align to 4-byte boundary
                        let pos = reader.position();
                        let aligned_pos = (pos + 3) & !3;
                        reader.seek(std::io::SeekFrom::Start(aligned_pos))?;
                        
                        // Read 3 default Vector3 values (first, middle, last keyframes)
                        let default_values: [Vector3; 3] = reader.read_le()?;
                        
                        // Read 9 additional floats (purpose unclear, possibly extended metadata)
                        let _additional_floats: [f32; 9] = reader.read_le()?;
                        
                        let remaining_data = data.elements.len() - (reader.position() as usize);
                        if remaining_data > 0 {
                            // Estimate bits per entry from remaining data
                            let total_bits = remaining_data * 8;
                            let bits_per_entry = if _compressed_frame_count > 0 {
                                (total_bits / _compressed_frame_count).max(1).min(72) // 3 components * 24 bits max
                            } else {
                                0
                            };

                            let compressed_frames = read_v12_compressed_vector3_data(
                                &mut reader, _compressed_frame_count, &default_values, bits_per_entry
                            )?;
                            for rotation_euler in compressed_frames {
                                let rotation = euler_to_quaternion(rotation_euler);
                                property_data.rotations.push(rotation);
                            }
                        } else {
                            // No compressed data, use default values
                            let default_euler = default_values[0];
                            let rotation = euler_to_quaternion(default_euler);
                            property_data.rotations.push(rotation);
                        }
                    }
                    0x0944 | 0x4409 => {
                        // Version 1.2: Compressed Vector4-based rotation data (quaternions)
                        // Format 0x0944 from GitHub discussion
                        let _compressed_frame_count = reader.read_le::<u32>()? as usize;
                        let _unk1 = reader.read_le::<f32>()?;
                        let _unk2 = reader.read_le::<f32>()?;
                        let _flags = reader.read_le::<u16>()?;
                        let bits_per_entry = reader.read_le::<u16>()? as usize;
                        
                        // Three default Vector4 quaternion values (first, middle, last)
                        let default_values: [Vector4; 3] = reader.read_le()?;
                        
                        let remaining_data = data.elements.len() - (reader.position() as usize);
                        if remaining_data > 0 && bits_per_entry > 0 {
                            let compressed_frames = read_v12_compressed_vector4_data(
                                &mut reader, _compressed_frame_count, &default_values, bits_per_entry
                            )?;
                            property_data.rotations.extend(compressed_frames);
                        } else {
                            // No compressed data, use default value
                            property_data.rotations.push(default_values[0]);
                        }
                    }
                    0x4003 => {
                        // Single Vector4 (quaternion rotation) - uncompressed
                        let rotation = reader.read_le::<Vector4>()?;
                        property_data.rotations.push(rotation);
                    }
                    0x3003 => {
                        // Single Vector3 (euler angles, convert to quaternion) - uncompressed
                        let euler: Vector3 = reader.read_le()?;
                        let rotation = euler_to_quaternion(euler);
                        property_data.rotations.push(rotation);
                    }
                    _ => {
                        // Unknown format, use default identity rotation
                        eprintln!("Warning: Unknown Rotate format header: 0x{:04X} for track {}", header, track.name.to_string_lossy());
                        property_data.rotations.push(Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 });
                    }
                }
            }
            "Translate" => {
                match header {
                    0x3003 => {
                        // Single Vector3 - uncompressed
                        let translation = reader.read_le::<Vector3>()?;
                        property_data.translations.push(translation);
                    }
                    0x3300 => {
                        // Variant of single Vector3 - different header but same structure
                        let translation = reader.read_le::<Vector3>()?;
                        property_data.translations.push(translation);
                    }
                    0x0934 | 0x3409 => {
                        // Version 1.2: Compressed Vector3 translation data
                        // Format 0x0934 from GitHub discussion
                        let _compressed_frame_count = reader.read_le::<u32>()? as usize;
                        let _unk1 = reader.read_le::<f32>()?;  // typically 1.0
                        let _unk2 = reader.read_le::<f32>()?;  // varies
                        let _flags = reader.read_le::<u16>()?; // typically 2
                        let bits_per_entry = reader.read_le::<u16>()? as usize;  // bit count for decompression
                        
                        // Three default Vector3 values (first frame, middle frame, last frame)
                        let default_values: [Vector3; 3] = reader.read_le()?;
                        
                        let remaining_data = data.elements.len() - (reader.position() as usize);
                        if remaining_data > 0 && bits_per_entry > 0 {
                            let compressed_frames = read_v12_compressed_vector3_data(
                                &mut reader, _compressed_frame_count, &default_values, bits_per_entry
                            )?;
                            property_data.translations.extend(compressed_frames);
                        } else {
                            // No compressed data, use default value
                            property_data.translations.push(default_values[0]);
                        }
                    }
                    0x4308 => {
                        // Version 1.2: Compressed Vector3 data with frame indices
                        let _compressed_frame_count = reader.read_le::<u32>()? as usize;
                        let _unk1 = reader.read_le::<f32>()?;

                        // Read frame indices (one byte per frame)
                        let mut frame_indices = vec![0u8; _compressed_frame_count];
                        reader.read_exact(&mut frame_indices)?;
                        
                        // Align to 4-byte boundary
                        let pos = reader.position();
                        let aligned_pos = (pos + 3) & !3;
                        reader.seek(std::io::SeekFrom::Start(aligned_pos))?;
                        
                        // Read 3 default Vector3 values (first, middle, last keyframes)
                        let default_values: [Vector3; 3] = reader.read_le()?;
                        
                        // Read 9 additional floats
                        let _additional_floats: [f32; 9] = reader.read_le()?;
                        
                        let remaining_data = data.elements.len() - (reader.position() as usize);
                        if remaining_data > 0 {
                            // Estimate bits per entry from remaining data
                            let total_bits = remaining_data * 8;
                            let bits_per_entry = if _compressed_frame_count > 0 {
                                (total_bits / _compressed_frame_count).max(1).min(72)
                            } else {
                                0
                            };

                            let compressed_frames = read_v12_compressed_vector3_data(
                                &mut reader, _compressed_frame_count, &default_values, bits_per_entry
                            )?;
                            property_data.translations.extend(compressed_frames);
                        } else {
                            // No compressed data, use default value
                            property_data.translations.push(default_values[0]);
                        }
                    }
                    _ => {
                        // Unknown format - log for debugging
                        eprintln!("Warning: Unknown Translate format header: 0x{:04X} for track {}", header, track.name.to_string_lossy());
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

// Read compressed Vector3 data from version 1.2 animation
// Based on GitHub discussion about version 1.2 compression format
// Version 1.2 uses a different compression scheme than version 2.0+
fn read_v12_compressed_vector3_data(
    reader: &mut Cursor<&Vec<u8>>,
    frame_count: usize,
    default_values: &[Vector3; 3],
    bits_per_entry: usize,
) -> Result<Vec<Vector3>, error::Error> {
    let remaining_bytes = reader.get_ref().len() - reader.position() as usize;

    // If no compressed data or no bits, interpolate between default values
    if remaining_bytes == 0 || bits_per_entry == 0 {
        return Ok(interpolate_vector3_frames(frame_count, default_values));
    }

    // Check if this might be uncompressed float data (12 bytes per Vector3)
    if remaining_bytes >= frame_count * 12 && bits_per_entry >= 32 * 3 {
        // Likely uncompressed: try reading as raw float data
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            if reader.position() as usize + 12 <= reader.get_ref().len() {
                let x = reader.read_le::<f32>()?;
                let y = reader.read_le::<f32>()?;
                let z = reader.read_le::<f32>()?;
                frames.push(Vector3 { x, y, z });
            } else {
                frames.push(default_values[0]);
            }
        }
        return Ok(frames);
    }

    // Version 1.2 compression: values are indices into interpolated ranges
    // The compressed data contains indices that select interpolation points between the 3 default values
    let mut compressed_data = vec![0u8; remaining_bytes];
    reader.read_exact(&mut compressed_data)?;

    let mut bit_reader = bitutils::BitReader::from_slice(&compressed_data);
    let mut frames = Vec::with_capacity(frame_count);

    // For version 1.2, bits_per_entry represents the total bits used for the entire Vector3
    // We need to determine how many bits are used per component
    let bits_per_component = match bits_per_entry {
        0 => 0,
        1..=8 => 8,  // Use 8 bits per component for low bit counts
        9..=16 => 8,
        17..=24 => 8,
        _ => 8,  // Cap at 8 bits per component for safety
    };

    // If we can't determine the bit count, fall back to interpolation
    if bits_per_component == 0 {
        return Ok(interpolate_vector3_frames(frame_count, default_values));
    }

    for i in 0..frame_count {
        let result = (|| -> Result<Vector3, error::Error> {
            // Read compressed indices for each component
            let x_index = bit_reader.read_u32(bits_per_component)?;
            let y_index = bit_reader.read_u32(bits_per_component)?;
            let z_index = bit_reader.read_u32(bits_per_component)?;

            // Convert indices to interpolation factors (0.0 to 1.0)
            let max_index = (1u32 << bits_per_component) - 1;
            let x_t = if max_index > 0 { x_index as f32 / max_index as f32 } else { 0.0 };
            let y_t = if max_index > 0 { y_index as f32 / max_index as f32 } else { 0.0 };
            let z_t = if max_index > 0 { z_index as f32 / max_index as f32 } else { 0.0 };

            // Interpolate between the three default values based on the indices
            // This is a simplified version - the actual algorithm may be more complex
            let x = interpolate_component(default_values[0].x, default_values[1].x, default_values[2].x, x_t);
            let y = interpolate_component(default_values[0].y, default_values[1].y, default_values[2].y, y_t);
            let z = interpolate_component(default_values[0].z, default_values[1].z, default_values[2].z, z_t);

            Ok(Vector3 { x, y, z })
        })();

        match result {
            Ok(vec) => frames.push(vec),
            Err(_) => {
                // If decompression fails, fall back to interpolation for remaining frames
                let interpolated = interpolate_vector3_frames(frame_count - i, default_values);
                frames.extend(interpolated);
                break;
            }
        }
    }

    Ok(frames)
}

// Read compressed Vector4 data from version 1.2 animation (for quaternions)
// Based on GitHub discussion about version 1.2 compression format
// Version 1.2 uses a different compression scheme than version 2.0+
fn read_v12_compressed_vector4_data(
    reader: &mut Cursor<&Vec<u8>>,
    frame_count: usize,
    default_values: &[Vector4; 3],
    bits_per_entry: usize,
) -> Result<Vec<Vector4>, error::Error> {
    let remaining_bytes = reader.get_ref().len() - reader.position() as usize;

    // If no compressed data or no bits, interpolate between default values
    if remaining_bytes == 0 || bits_per_entry == 0 {
        return Ok(interpolate_vector4_frames(frame_count, default_values));
    }

    // Check if this might be uncompressed float data (16 bytes per Vector4/quaternion)
    if remaining_bytes >= frame_count * 16 && bits_per_entry >= 32 * 4 {
        // Likely uncompressed: try reading as raw float data
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            if reader.position() as usize + 16 <= reader.get_ref().len() {
                let x = reader.read_le::<f32>()?;
                let y = reader.read_le::<f32>()?;
                let z = reader.read_le::<f32>()?;
                let w = reader.read_le::<f32>()?;

                // Normalize quaternion
                let length = (x * x + y * y + z * z + w * w).sqrt();
                if length > 0.001 {
                    frames.push(Vector4 {
                        x: x / length,
                        y: y / length,
                        z: z / length,
                        w: w / length
                    });
                } else {
                    frames.push(default_values[0]);
                }
            } else {
                frames.push(default_values[0]);
            }
        }
        return Ok(frames);
    }

    // Version 1.2 quaternion compression: similar to Vector3 but with quaternion normalization
    let mut compressed_data = vec![0u8; remaining_bytes];
    reader.read_exact(&mut compressed_data)?;

    let mut bit_reader = bitutils::BitReader::from_slice(&compressed_data);
    let mut frames = Vec::with_capacity(frame_count);

    // For version 1.2, determine bits per component based on bits_per_entry
    let bits_per_component = match bits_per_entry {
        0 => 0,
        1..=12 => 4,  // Use fewer bits for quaternions
        13..=24 => 6,
        25..=36 => 8,
        _ => 8,  // Cap at 8 bits per component
    };

    // If we can't determine the bit count, fall back to interpolation
    if bits_per_component == 0 {
        return Ok(interpolate_vector4_frames(frame_count, default_values));
    }

    for i in 0..frame_count {
        let result = (|| -> Result<Vector4, error::Error> {
            // Read compressed indices for X, Y, Z components
            let x_index = bit_reader.read_u32(bits_per_component)?;
            let y_index = bit_reader.read_u32(bits_per_component)?;
            let z_index = bit_reader.read_u32(bits_per_component)?;

            // For quaternions, we may also have a W sign bit
            let w_sign_bit = if bits_per_entry > bits_per_component * 3 {
                bit_reader.read_bit().unwrap_or(true)
            } else {
                true
            };

            // Convert indices to interpolation factors (0.0 to 1.0)
            let max_index = (1u32 << bits_per_component) - 1;
            let x_t = if max_index > 0 { x_index as f32 / max_index as f32 } else { 0.0 };
            let y_t = if max_index > 0 { y_index as f32 / max_index as f32 } else { 0.0 };
            let z_t = if max_index > 0 { z_index as f32 / max_index as f32 } else { 0.0 };

            // Interpolate quaternion components
            let x = interpolate_component(default_values[0].x, default_values[1].x, default_values[2].x, x_t);
            let y = interpolate_component(default_values[0].y, default_values[1].y, default_values[2].y, y_t);
            let z = interpolate_component(default_values[0].z, default_values[1].z, default_values[2].z, z_t);
            let w_base = interpolate_component(default_values[0].w, default_values[1].w, default_values[2].w, (x_t + y_t + z_t) / 3.0);

            // Apply sign bit to W component
            let w = if w_sign_bit { w_base.abs() } else { -w_base.abs() };

            // Normalize the quaternion
            let length = (x * x + y * y + z * z + w * w).sqrt();
            if length > 0.001 {
                Ok(Vector4 {
                    x: x / length,
                    y: y / length,
                    z: z / length,
                    w: w / length,
                })
            } else {
                Ok(default_values[0])
            }
        })();

        match result {
            Ok(quat) => frames.push(quat),
            Err(_) => {
                // If decompression fails, fall back to interpolation for remaining frames
                let interpolated = interpolate_vector4_frames(frame_count - i, default_values);
                frames.extend(interpolated);
                break;
            }
        }
    }

    Ok(frames)
}

// Helper function to interpolate a single component between three values
fn interpolate_component(start: f32, middle: f32, end: f32, t: f32) -> f32 {
    // For version 1.2, we use a simple interpolation scheme
    // This may need to be adjusted based on the actual algorithm used in the game
    if t <= 0.5 {
        // Interpolate between start and middle
        let local_t = t * 2.0;
        start + (middle - start) * local_t
    } else {
        // Interpolate between middle and end
        let local_t = (t - 0.5) * 2.0;
        middle + (end - middle) * local_t
    }
}

// Helper function to interpolate Vector3 values between key frames
fn interpolate_vector3_frames(frame_count: usize, default_values: &[Vector3; 3]) -> Vec<Vector3> {
    if frame_count == 0 {
        return Vec::new();
    }

    if frame_count == 1 {
        return vec![default_values[0]];
    }

    let mut frames = Vec::with_capacity(frame_count);
    let first = default_values[0];
    let middle = default_values[1];
    let last = default_values[2];

    // Determine middle frame index
    let middle_frame = frame_count / 2;

    for i in 0..frame_count {
        let value = if i == 0 {
            first
        } else if i == frame_count - 1 {
            last
        } else if i == middle_frame {
            middle
        } else if i < middle_frame {
            // Interpolate between first and middle
            let t = i as f32 / middle_frame as f32;
            Vector3 {
                x: first.x + (middle.x - first.x) * t,
                y: first.y + (middle.y - first.y) * t,
                z: first.z + (middle.z - first.z) * t,
            }
        } else {
            // Interpolate between middle and last
            let t = (i - middle_frame) as f32 / (frame_count - 1 - middle_frame) as f32;
            Vector3 {
                x: middle.x + (last.x - middle.x) * t,
                y: middle.y + (last.y - middle.y) * t,
                z: middle.z + (last.z - middle.z) * t,
            }
        };
        frames.push(value);
    }

    frames
}



// Helper function to interpolate Vector4 (quaternion) values between key frames
fn interpolate_vector4_frames(frame_count: usize, default_values: &[Vector4; 3]) -> Vec<Vector4> {
    if frame_count == 0 {
        return Vec::new();
    }
    
    if frame_count == 1 {
        return vec![default_values[0]];
    }
    
    let mut frames = Vec::with_capacity(frame_count);
    let first = default_values[0];
    let middle = default_values[1];
    let last = default_values[2];
    
    let middle_frame = frame_count / 2;
    
    for i in 0..frame_count {
        let value = if i == 0 {
            first
        } else if i == frame_count - 1 {
            last
        } else if i == middle_frame {
            middle
        } else if i < middle_frame {
            // Interpolate between first and middle (simple lerp for now)
            let t = i as f32 / middle_frame as f32;
            slerp_quaternion(first, middle, t)
        } else {
            // Interpolate between middle and last
            let t = (i - middle_frame) as f32 / (frame_count - 1 - middle_frame) as f32;
            slerp_quaternion(middle, last, t)
        };
        frames.push(value);
    }
    
    frames
}

// Spherical linear interpolation for quaternions
fn slerp_quaternion(q1: Vector4, q2: Vector4, t: f32) -> Vector4 {
    // Calculate dot product
    let dot = q1.x * q2.x + q1.y * q2.y + q1.z * q2.z + q1.w * q2.w;
    
    // If quaternions are very close, use linear interpolation
    if dot.abs() > 0.9995 {
        let result = Vector4 {
            x: q1.x + (q2.x - q1.x) * t,
            y: q1.y + (q2.y - q1.y) * t,
            z: q1.z + (q2.z - q1.z) * t,
            w: q1.w + (q2.w - q1.w) * t,
        };
        let length = (result.x * result.x + result.y * result.y + result.z * result.z + result.w * result.w).sqrt();
        return Vector4 {
            x: result.x / length,
            y: result.y / length,
            z: result.z / length,
            w: result.w / length,
        };
    }
    
    // Use the shorter path
    let (q2_adjusted, dot_adjusted) = if dot < 0.0 {
        (Vector4 { x: -q2.x, y: -q2.y, z: -q2.z, w: -q2.w }, -dot)
    } else {
        (q2, dot)
    };
    
    let theta = dot_adjusted.acos();
    let sin_theta = theta.sin();
    
    if sin_theta.abs() < 0.001 {
        // Fallback to linear interpolation
        let result = Vector4 {
            x: q1.x + (q2_adjusted.x - q1.x) * t,
            y: q1.y + (q2_adjusted.y - q1.y) * t,
            z: q1.z + (q2_adjusted.z - q1.z) * t,
            w: q1.w + (q2_adjusted.w - q1.w) * t,
        };
        let length = (result.x * result.x + result.y * result.y + result.z * result.z + result.w * result.w).sqrt();
        return Vector4 {
            x: result.x / length,
            y: result.y / length,
            z: result.z / length,
            w: result.w / length,
        };
    }
    
    let a = ((1.0 - t) * theta).sin() / sin_theta;
    let b = (t * theta).sin() / sin_theta;
    
    Vector4 {
        x: q1.x * a + q2_adjusted.x * b,
        y: q1.y * a + q2_adjusted.y * b,
        z: q1.z * a + q2_adjusted.z * b,
        w: q1.w * a + q2_adjusted.w * b,
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
// Vector3?
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

// Vector4?
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

// Helper functions for version 1.2 creation

fn create_v12_compressed_vector3_data(values: &[Vector3]) -> Result<Vec<u8>, error::Error> {
    let frame_count = values.len() as u32;
    let mut data = Vec::new();
    
    // Use format 0x3409 for compressed Vector3 data
    data.extend_from_slice(&0x3409u32.to_le_bytes());
    data.extend_from_slice(&frame_count.to_le_bytes());
    data.extend_from_slice(&0.0f32.to_le_bytes()); // unk1
    data.extend_from_slice(&0.0f32.to_le_bytes()); // unk2
    data.extend_from_slice(&0u16.to_le_bytes());   // flags
    data.extend_from_slice(&0u16.to_le_bytes());   // padding
    
    // Default values (use first, min, max pattern as seen in the read logic)
    let default_value = if !values.is_empty() { values[0] } else { Vector3 { x: 0.0, y: 0.0, z: 0.0 } };
    data.extend_from_slice(&default_value.x.to_le_bytes());
    data.extend_from_slice(&default_value.y.to_le_bytes());
    data.extend_from_slice(&default_value.z.to_le_bytes());
    
    // Add two more default values (as seen in the read logic pattern)
    data.extend_from_slice(&default_value.x.to_le_bytes());
    data.extend_from_slice(&default_value.y.to_le_bytes());
    data.extend_from_slice(&default_value.z.to_le_bytes());
    data.extend_from_slice(&default_value.x.to_le_bytes());
    data.extend_from_slice(&default_value.y.to_le_bytes());
    data.extend_from_slice(&default_value.z.to_le_bytes());
    
    // Write raw frame data
    for value in values {
        data.extend_from_slice(&value.x.to_le_bytes());
        data.extend_from_slice(&value.y.to_le_bytes());
        data.extend_from_slice(&value.z.to_le_bytes());
    }
    
    Ok(data)
}

fn create_v12_compressed_vector4_data(values: &[Vector4]) -> Result<Vec<u8>, error::Error> {
    let frame_count = values.len() as u32;
    let mut data = Vec::new();
    
    // Use format 0x4409 for compressed Vector4 data
    data.extend_from_slice(&0x4409u32.to_le_bytes());
    data.extend_from_slice(&frame_count.to_le_bytes());
    data.extend_from_slice(&0.0f32.to_le_bytes()); // unk1
    data.extend_from_slice(&0.0f32.to_le_bytes()); // unk2
    data.extend_from_slice(&0u16.to_le_bytes());   // flags
    data.extend_from_slice(&0u16.to_le_bytes());   // padding
    
    // Default values (use first, identity, identity pattern)
    let default_value = if !values.is_empty() { values[0] } else { Vector4 { x: 0.0, y: 0.0, z: 0.0, w: 1.0 } };
    data.extend_from_slice(&default_value.x.to_le_bytes());
    data.extend_from_slice(&default_value.y.to_le_bytes());
    data.extend_from_slice(&default_value.z.to_le_bytes());
    data.extend_from_slice(&default_value.w.to_le_bytes());
    
    // Add two more default values
    data.extend_from_slice(&default_value.x.to_le_bytes());
    data.extend_from_slice(&default_value.y.to_le_bytes());
    data.extend_from_slice(&default_value.z.to_le_bytes());
    data.extend_from_slice(&default_value.w.to_le_bytes());
    data.extend_from_slice(&default_value.x.to_le_bytes());
    data.extend_from_slice(&default_value.y.to_le_bytes());
    data.extend_from_slice(&default_value.z.to_le_bytes());
    data.extend_from_slice(&default_value.w.to_le_bytes());
    
    // Write raw frame data
    for value in values {
        data.extend_from_slice(&value.x.to_le_bytes());
        data.extend_from_slice(&value.y.to_le_bytes());
        data.extend_from_slice(&value.z.to_le_bytes());
        data.extend_from_slice(&value.w.to_le_bytes());
    }
    
    Ok(data)
}

fn create_v12_compressed_bool_data(values: &[bool]) -> Result<Vec<u8>, error::Error> {
    let _frame_count = values.len() as u32;
    let mut data = Vec::new();
    
    // Use a simple format for boolean data
    data.extend_from_slice(&0x1013u32.to_le_bytes());
    
    // For now, just use the first value if all are the same
    // For complex boolean animations, we'd need a more sophisticated format
    let value = if !values.is_empty() { values[0] } else { true };
    data.extend_from_slice(&(if value { 1u16 } else { 0u16 }).to_le_bytes());
    
    Ok(data)
}

fn create_v12_compressed_uv_data(values: &[UvTransform]) -> Result<Vec<u8>, error::Error> {
    let _frame_count = values.len() as u32;
    let mut data = Vec::new();
    
    // Use simple format for UV transforms
    data.extend_from_slice(&0x5014u32.to_le_bytes());
    
    // For now, just use the first value
    let uv = if !values.is_empty() { &values[0] } else { &UvTransform::default() };
    data.extend_from_slice(&uv.scale_u.to_le_bytes());
    data.extend_from_slice(&uv.scale_v.to_le_bytes());
    data.extend_from_slice(&uv.rotation.to_le_bytes());
    data.extend_from_slice(&uv.translate_u.to_le_bytes());
    data.extend_from_slice(&uv.translate_v.to_le_bytes());
    
    Ok(data)
}
