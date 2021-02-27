use std::error::Error;

use binread::io::{Seek, SeekFrom};
use binread::BinReaderExt;
use binread::{io::Cursor, BinRead};
use ssbh_lib::formats::mesh::{
    AttributeDataType, AttributeDataTypeV8, AttributeUsage, Mesh, MeshAttributeV10,
    MeshAttributeV8, MeshObject, MeshRiggingGroup,
};
use ssbh_lib::Half;

pub enum DataType {
    Byte,
    Float,
    HalfFloat,
}

#[derive(BinRead, Debug)]
pub struct VertexWeight {
    vertex_index: i16,
    vertex_weight: f32,
}

impl From<AttributeDataType> for DataType {
    fn from(value: AttributeDataType) -> Self {
        match value {
            AttributeDataType::Float => Self::Float,
            AttributeDataType::Byte => Self::Byte,
            AttributeDataType::HalfFloat => Self::HalfFloat,
            AttributeDataType::HalfFloat2 => Self::HalfFloat,
        }
    }
}

impl From<AttributeDataTypeV8> for DataType {
    fn from(value: AttributeDataTypeV8) -> Self {
        match value {
            AttributeDataTypeV8::Float => Self::Float,
            AttributeDataTypeV8::Float2 => Self::Float,
            AttributeDataTypeV8::Byte => Self::Byte,
            AttributeDataTypeV8::HalfFloat => Self::HalfFloat,
        }
    }
}

/// Read the vertex indices from the buffer in `mesh` for the specified `mesh_object`.
/// Index values are converted to `u32` regardless of the actual data type.
pub fn read_vertex_indices(
    mesh: &Mesh,
    mesh_object: &MeshObject,
) -> Result<Vec<u32>, Box<dyn Error>> {
    let mut indices = Vec::new();

    let mut reader = Cursor::new(&mesh.polygon_buffer.elements);
    reader.seek(SeekFrom::Start(mesh_object.element_offset as u64))?;

    match mesh_object.draw_element_type {
        ssbh_lib::formats::mesh::DrawElementType::UnsignedShort => {
            for _ in 0..mesh_object.vertex_index_count {
                let index = reader.read_le::<u16>()? as u32;
                indices.push(index);
            }
        }
        ssbh_lib::formats::mesh::DrawElementType::UnsignedInt => {
            for _ in 0..mesh_object.vertex_index_count {
                let index = reader.read_le::<u32>()?;
                indices.push(index);
            }
        }
    }

    Ok(indices)
}

macro_rules! read_attribute_data {
    ($mesh:expr,$mesh_object:expr,$buffer_access:expr,$t_out:ty,$size:expr) => {{
        // Get the raw data for the attribute for this mesh object.
        let attribute_buffer = $mesh
            .vertex_buffers
            .elements
            .get($buffer_access.index as usize)
            .ok_or("Invalid buffer index.")?;

        // TODO: Handle invalid indices and return some sort of error.
        // TODO: Create functions for this?
        let offset = match $buffer_access.index {
            0 => Ok($buffer_access.offset + $mesh_object.vertex_offset as u64),
            1 => Ok($buffer_access.offset + $mesh_object.vertex_offset2 as u64),
            _ => Err("Buffer indices higher than 1 are not supported."),
        }? as u64;

        let stride = match $buffer_access.index {
            0 => Ok($mesh_object.stride),
            1 => Ok($mesh_object.stride2),
            _ => Err("Buffer indices higher than 1 are not supported."),
        }? as u64;

        let count = $mesh_object.vertex_count as usize;

        let mut reader = Cursor::new(&attribute_buffer.elements);

        let data = match $buffer_access.data_type {
            DataType::Byte => read_data!(reader, count, offset, stride, u8, $t_out, $size),
            DataType::Float => read_data!(reader, count, offset, stride, f32, $t_out, $size),
            DataType::HalfFloat => read_data!(reader, count, offset, stride, Half, $t_out, $size),
        };

        data
    }};
}

macro_rules! read_data {
    ($reader:expr,$count:expr,$offset:expr,$stride:expr,$t_in:ty,$t_out:ty,$size:expr) => {{
        $reader.seek(SeekFrom::Start($offset))?;

        let mut result = Vec::new();
        for i in 0..$count as u64 {
            // The data type may be smaller than stride to allow interleaving different attributes.
            $reader.seek(SeekFrom::Start($offset + i * $stride))?;

            let mut element = [<$t_out>::default(); $size];
            for j in 0..$size {
                element[j] = <$t_out>::from($reader.read_le::<$t_in>()?);
            }
            result.push(element);
        }
        result
    }};
}

/// Read the vertex positions for the specified `mesh_object`.
pub fn read_positions(
    mesh: &Mesh,
    mesh_object: &MeshObject,
) -> Result<Vec<[f32; 3]>, Box<dyn Error>> {
    let attributes = get_attributes(&mesh_object, AttributeUsage::Position);
    let buffer_access = attributes.first().ok_or("No position attribute found.")?;
    let data = read_attribute_data!(mesh, mesh_object, buffer_access, f32, 3);
    Ok(data)
}

/// Returns all the texture coordinate attributes for the specified `mesh_object`.
pub fn read_texture_coordinates(
    mesh: &Mesh,
    mesh_object: &MeshObject,
) -> Result<Vec<Vec<[f32; 2]>>, Box<dyn Error>> {
    let mut attributes = Vec::new();
    for buffer_access in get_attributes(&mesh_object, AttributeUsage::TextureCoordinate) {
        let data = read_attribute_data!(mesh, mesh_object, buffer_access, f32, 2);
        attributes.push(data);
    }

    Ok(attributes)
}

/// Returns all the colorset attributes for the specified `mesh_object`.
pub fn read_colorsets(
    mesh: &Mesh,
    mesh_object: &MeshObject,
) -> Result<Vec<Vec<[f32; 4]>>, Box<dyn Error>> {
    // TODO: Find a cleaner way to do this (define a new enum?).
    let colorsets_v10 = get_attributes(&mesh_object, AttributeUsage::ColorSet);
    let colorsets_v8 = get_attributes(&mesh_object, AttributeUsage::ColorSetV8);

    let mut attributes = Vec::new();
    for buffer_access in colorsets_v10.iter().chain(colorsets_v8.iter()) {
        let data = read_attribute_data!(mesh, mesh_object, buffer_access, f32, 4);
        attributes.push(data);
    }

    Ok(attributes)
}

pub fn read_normals(
    mesh: &Mesh,
    mesh_object: &MeshObject,
) -> Result<Vec<[f32; 3]>, Box<dyn Error>> {
    let attributes = get_attributes(&mesh_object, AttributeUsage::Normal);
    let buffer_access = attributes.first().ok_or("No normals attribute found.")?;
    let data = read_attribute_data!(mesh, mesh_object, buffer_access, f32, 3);
    Ok(data)
}

#[derive(Debug)]
pub struct MeshObjectRiggingData {
    pub mesh_object_name: String,
    pub mesh_sub_index: u64,
    pub bone_influences: Vec<BoneInfluence>,
}

/// Reads the rigging data for the specified `mesh`. Rigging data is not a indluded with the `MeshObject`,
/// so each element of the output will need to be associated with the `MeshObject` with matching `name` and `sub_index`.
/// Each vertex will likely be influenced by at most 4 bones, but the format doesn't enforce this.
pub fn read_rigging_data(mesh: &Mesh) -> Result<Vec<MeshObjectRiggingData>, Box<dyn Error>> {
    let mut mesh_object_rigging_data = Vec::new();

    for rigging_group in &mesh.rigging_buffers.elements {
        let mesh_object_name = rigging_group
            .mesh_object_name
            .get_string()
            .ok_or("Failed to read mesh object name.")?;
        let mesh_sub_index = rigging_group.mesh_object_sub_index;

        let bone_influences = read_influences(&rigging_group)?;

        // TODO: Store max influences?
        let rigging_data = MeshObjectRiggingData {
            mesh_object_name: mesh_object_name.to_string(),
            mesh_sub_index,
            bone_influences,
        };

        mesh_object_rigging_data.push(rigging_data);
    }

    Ok(mesh_object_rigging_data)
}

#[derive(Debug)]
pub struct BoneInfluence {
    pub bone_name: String,
    pub vertex_weights: Vec<VertexWeight>,
}

fn read_influences(rigging_group: &MeshRiggingGroup) -> Result<Vec<BoneInfluence>, Box<dyn Error>> {
    let mut bone_influences = Vec::new();
    for buffer in &rigging_group.buffers.elements {
        let bone_name = buffer
            .bone_name
            .get_string()
            .ok_or("Failed to read bone name.")?;

        // TODO: Is there a way to do this with iterators?
        // There's no stored length for the buffer, so read influences until reaching the end.
        let mut influences = Vec::new();
        let mut reader = Cursor::new(&buffer.data.elements);
        while let Ok(influence) = reader.read_le::<VertexWeight>() {
            influences.push(influence);
        }

        let bone_influence = BoneInfluence {
            bone_name: bone_name.to_string(),
            vertex_weights: influences,
        };
        bone_influences.push(bone_influence);
    }

    Ok(bone_influences)
}

struct BufferAccess {
    pub index: u64,
    pub offset: u64,
    pub data_type: DataType,
}

impl From<&MeshAttributeV10> for BufferAccess {
    fn from(a: &MeshAttributeV10) -> Self {
        BufferAccess {
            index: a.buffer_index as u64,
            offset: a.buffer_offset as u64,
            data_type: a.data_type.into(),
        }
    }
}

impl From<&MeshAttributeV8> for BufferAccess {
    fn from(a: &MeshAttributeV8) -> Self {
        BufferAccess {
            index: a.buffer_index as u64,
            offset: a.buffer_offset as u64,
            data_type: a.data_type.into(),
        }
    }
}

fn get_attributes(mesh_object: &MeshObject, usage: AttributeUsage) -> Vec<BufferAccess> {
    match &mesh_object.attributes {
        ssbh_lib::formats::mesh::MeshAttributes::AttributesV8(attributes_v8) => attributes_v8
            .elements
            .iter()
            .filter(|a| a.usage == usage)
            .map(|a| a.into())
            .collect(),
        ssbh_lib::formats::mesh::MeshAttributes::AttributesV10(attributes_v10) => attributes_v10
            .elements
            .iter()
            .filter(|a| a.usage == usage)
            .map(|a| a.into())
            .collect(),
    }
}

/// Gets the name of the mesh attribute. This uses the attribute names array,
/// which can be assumed to contain a single value that is unique with respect to the other attributes for the mesh object.
pub fn get_attribute_name(attribute: &MeshAttributeV10) -> Option<&str> {
    attribute
        .attribute_names
        .elements
        .iter()
        .next()
        .unwrap()
        .get_string()
}
