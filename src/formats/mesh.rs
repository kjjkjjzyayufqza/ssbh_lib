use crate::{Matrix3x3, SsbhString8};
use crate::SsbhArray;
use crate::SsbhByteBuffer;
use crate::SsbhString;
use crate::Vector3;
use modular_bitfield::prelude::*;

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};
use ssbh_write_derive::SsbhWrite;

use binread::BinRead;

/// The vertex buffers and associated geometric data for a mesh.
/// Compatible with file version 1.8 and 1.10.
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
#[align_after(8)]
pub struct Mesh {
    pub major_version: u16,
    pub minor_version: u16,
    pub model_name: SsbhString,
    pub bounding_info: BoundingInfo,
    pub unk1: u32, // always 0
    #[br(args(major_version, minor_version))]
    pub objects: SsbhArray<MeshObject>,
    pub buffer_sizes: SsbhArray<u32>,
    pub polygon_index_size: u64,
    pub vertex_buffers: SsbhArray<SsbhByteBuffer>,
    pub polygon_buffer: SsbhByteBuffer,
    pub rigging_buffers: SsbhArray<MeshRiggingGroup>,
    pub unknown_offset: u64, // these are probably just padding
    pub unknown_size: u64,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct MeshAttributeV10 {
    pub usage: AttributeUsage,
    pub data_type: AttributeDataType,
    pub buffer_index: u32,
    pub buffer_offset: u32,
    /// The index for multiple attributes of the same usage starting from 0.
    pub sub_index: u64,
    pub name: SsbhString,
    pub attribute_names: SsbhArray<SsbhString>,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct BoundingInfo {
    pub bounding_sphere: BoundingSphere,
    pub bounding_volume: BoundingVolume,
    pub oriented_bounding_box: OrientedBoundingBox,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct BoundingSphere {
    pub center: Vector3,
    pub radius: f32,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct BoundingVolume {
    pub min: Vector3,
    pub max: Vector3,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct OrientedBoundingBox {
    pub center: Vector3,
    pub transform: Matrix3x3,
    pub size: Vector3,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct MeshAttributeV8 {
    pub usage: AttributeUsage,
    pub data_type: AttributeDataTypeV8,
    pub buffer_index: u32,
    pub buffer_offset: u32,
    /// The index for multiple attributes of the same usage starting from 0.
    pub sub_index: u32,
}

#[bitfield]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, Copy, Clone)]
#[br(map = Self::from_bytes)]
pub struct RiggingFlags {
    pub max_influences: B8,
    pub unk1: bool,
    #[skip]
    unused: B55,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct MeshBoneBuffer {
    pub bone_name: SsbhString,
    pub data: SsbhByteBuffer,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct MeshRiggingGroup {
    pub mesh_object_name: SsbhString,
    pub mesh_object_sub_index: u64,
    pub flags: RiggingFlags,
    pub buffers: SsbhArray<MeshBoneBuffer>,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug)]
#[br(import(major_version: u16, minor_version: u16))]
pub enum MeshAttributes {
    #[br(pre_assert(major_version == 1 &&  minor_version == 8))]
    AttributesV8(SsbhArray<MeshAttributeV8>),

    #[br(pre_assert(major_version == 1 &&  minor_version == 10))]
    AttributesV10(SsbhArray<MeshAttributeV10>),
}

#[br(import(major_version: u16, minor_version: u16))]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, SsbhWrite)]
pub struct MeshObject {
    pub name: SsbhString,
    pub sub_index: i64,
    pub parent_bone_name: SsbhString,
    pub vertex_count: u32,
    pub vertex_index_count: u32,
    pub unk2: u32, // number of indices per face (always 3)?
    pub vertex_offset: u32,
    pub vertex_offset2: u32,
    pub final_buffer_offset: u32,
    pub buffer_index: i32,
    pub stride: u32,
    pub stride2: u32,
    pub unk6: u32, // set to 32 for version 1.8 and 0 otherwise
    pub unk7: u32, // always 0
    pub element_offset: u32,
    pub unk8: u32, // always 4
    pub draw_element_type: DrawElementType,
    pub rigging_type: RiggingType,
    pub unk11: i32, // unk index
    pub unk12: u32, // unk flags (0,1,256,257)
    pub bounding_info: BoundingInfo,
    #[br(args(major_version, minor_version))]
    pub attributes: MeshAttributes,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, Clone, Copy)]
#[br(repr(u32))]
pub enum DrawElementType {
    UnsignedShort = 0,
    UnsignedInt = 1,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, Clone, Copy)]
#[br(repr(u32))]
pub enum RiggingType {
    SingleBound = 0,
    Weighted = 1,
}

/// The data type and component count for the attribute's data. 
/// This determines the stride and offset between attributes.
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, Clone, Copy)]
#[br(repr(u32))]
pub enum AttributeDataType {
    /// 3 component (xyz or rgb) 32 bit floating point data.
    Float3 = 0,
    /// 4 component (rgba) 8 bit unsigned integer data.
    Byte4 = 2,
    /// 4 component (xyzw or rgba) 16 bit floating point data.
    HalfFloat4 = 5,
    /// 2 component (xy or uv) 16 bit floating point data.
    HalfFloat2 = 8,
}

/// The data type and component count for the attribute's data. 
/// This determines the stride and offset between attributes.
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, Clone, Copy)]
#[br(repr(u32))]
pub enum AttributeDataTypeV8 {
    /// 3 component (xyz or rgb) 32 bit floating point data.
    Float3 = 820,
    /// 4 component (xyzw or rgba) 16 bit floating point data.
    HalfFloat4 = 1077,
    /// 2 component (xy or uv) 32 bit floating point data.
    Float2 = 1079,
    /// 4 component (rgba) 8 bit unsigned integer data.
    Byte4 = 1024,
}

#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug, Clone, Copy, PartialEq)]
#[br(repr(u32))]
pub enum AttributeUsage {
    Position = 0,
    Normal = 1,
    Tangent = 3,
    TextureCoordinate = 4,
    ColorSet = 5,
    ColorSetV8 = 8,
}
