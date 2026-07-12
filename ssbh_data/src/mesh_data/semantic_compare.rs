//! Test-only helpers for comparing mesh semantics across write/reparse round trips.
//!
//! Optimized output may differ byte-for-byte from legacy output, but it must not
//! differ in model behavior. These helpers compare the semantic fields listed in
//! the write-profile contract: versions, object identity and order, vertex
//! indices, attribute data with bit-exact `f32` values, and rigging data.
use super::{Attribute, MeshData, VectorData, Weight};
use ssbh_lib::formats::mesh::{Mesh, MeshInner};

/// Asserts that two [MeshData] values describe the same model semantics.
///
/// Mesh v1.8 files do not store attribute name strings, so reparsed names are
/// generated from usage conventions. Pass `compare_attribute_names: false` when
/// one side contains generated names.
pub(crate) fn assert_mesh_data_semantically_equal(
    expected: &MeshData,
    actual: &MeshData,
    compare_attribute_names: bool,
) {
    if let Some(difference) =
        mesh_data_semantic_difference(expected, actual, compare_attribute_names)
    {
        panic!("mesh data is not semantically equal: {difference}");
    }
}

/// Returns a description of the first semantic difference or [None] if equal.
pub(crate) fn mesh_data_semantic_difference(
    expected: &MeshData,
    actual: &MeshData,
    compare_attribute_names: bool,
) -> Option<String> {
    if (expected.major_version, expected.minor_version)
        != (actual.major_version, actual.minor_version)
    {
        return Some(format!(
            "version {}.{} != {}.{}",
            expected.major_version,
            expected.minor_version,
            actual.major_version,
            actual.minor_version
        ));
    }

    if expected.objects.len() != actual.objects.len() {
        return Some(format!(
            "object count {} != {}",
            expected.objects.len(),
            actual.objects.len()
        ));
    }

    for (i, (e, a)) in expected
        .objects
        .iter()
        .zip(actual.objects.iter())
        .enumerate()
    {
        let ctx = format!("object {} '{}' (subindex {})", i, e.name, e.subindex);

        if e.name != a.name {
            return Some(format!("{ctx}: name '{}' != '{}'", e.name, a.name));
        }
        if e.subindex != a.subindex {
            return Some(format!("{ctx}: subindex {} != {}", e.subindex, a.subindex));
        }
        if e.parent_bone_name != a.parent_bone_name {
            return Some(format!(
                "{ctx}: parent bone '{}' != '{}'",
                e.parent_bone_name, a.parent_bone_name
            ));
        }
        if e.sort_bias != a.sort_bias {
            return Some(format!(
                "{ctx}: sort bias {} != {}",
                e.sort_bias, a.sort_bias
            ));
        }
        if e.disable_depth_write != a.disable_depth_write {
            return Some(format!(
                "{ctx}: disable_depth_write {} != {}",
                e.disable_depth_write, a.disable_depth_write
            ));
        }
        if e.disable_depth_test != a.disable_depth_test {
            return Some(format!(
                "{ctx}: disable_depth_test {} != {}",
                e.disable_depth_test, a.disable_depth_test
            ));
        }

        if e.vertex_indices.len() != a.vertex_indices.len() {
            return Some(format!(
                "{ctx}: vertex index count {} != {}",
                e.vertex_indices.len(),
                a.vertex_indices.len()
            ));
        }
        if let Some(position) = e
            .vertex_indices
            .iter()
            .zip(a.vertex_indices.iter())
            .position(|(x, y)| x != y)
        {
            return Some(format!(
                "{ctx}: vertex index {} is {} != {}",
                position, e.vertex_indices[position], a.vertex_indices[position]
            ));
        }

        let groups = [
            ("positions", &e.positions, &a.positions),
            ("normals", &e.normals, &a.normals),
            ("binormals", &e.binormals, &a.binormals),
            ("tangents", &e.tangents, &a.tangents),
            (
                "texture_coordinates",
                &e.texture_coordinates,
                &a.texture_coordinates,
            ),
            ("color_sets", &e.color_sets, &a.color_sets),
        ];
        for (group, expected_attributes, actual_attributes) in groups {
            if expected_attributes.len() != actual_attributes.len() {
                return Some(format!(
                    "{ctx}: {group} attribute count {} != {}",
                    expected_attributes.len(),
                    actual_attributes.len()
                ));
            }
            for (j, (ea, aa)) in expected_attributes
                .iter()
                .zip(actual_attributes.iter())
                .enumerate()
            {
                let attribute_ctx = format!("{ctx}: {group}[{}] '{}'", j, ea.name);
                if compare_attribute_names && ea.name != aa.name {
                    return Some(format!(
                        "{attribute_ctx}: attribute name '{}' != '{}'",
                        ea.name, aa.name
                    ));
                }
                if let Some(difference) = vector_data_difference(&ea.data, &aa.data) {
                    return Some(format!("{attribute_ctx}: {difference}"));
                }
            }
        }

        if e.bone_influences.len() != a.bone_influences.len() {
            return Some(format!(
                "{ctx}: bone influence count {} != {}",
                e.bone_influences.len(),
                a.bone_influences.len()
            ));
        }
        for (j, (ei, ai)) in e
            .bone_influences
            .iter()
            .zip(a.bone_influences.iter())
            .enumerate()
        {
            let influence_ctx = format!("{ctx}: bone influence [{}] '{}'", j, ei.bone_name);
            if ei.bone_name != ai.bone_name {
                return Some(format!(
                    "{influence_ctx}: bone name '{}' != '{}'",
                    ei.bone_name, ai.bone_name
                ));
            }
            if ei.vertex_weights.len() != ai.vertex_weights.len() {
                return Some(format!(
                    "{influence_ctx}: weight count {} != {}",
                    ei.vertex_weights.len(),
                    ai.vertex_weights.len()
                ));
            }
            for (k, (ew, aw)) in ei
                .vertex_weights
                .iter()
                .zip(ai.vertex_weights.iter())
                .enumerate()
            {
                if ew.vertex_index != aw.vertex_index {
                    return Some(format!(
                        "{influence_ctx}: weight [{}] vertex index {} != {}",
                        k, ew.vertex_index, aw.vertex_index
                    ));
                }
                if ew.vertex_weight.to_bits() != aw.vertex_weight.to_bits() {
                    return Some(format!(
                        "{influence_ctx}: weight [{}] value {} != {}",
                        k, ew.vertex_weight, aw.vertex_weight
                    ));
                }
            }
        }
    }

    None
}

fn vector_data_difference(expected: &VectorData, actual: &VectorData) -> Option<String> {
    let (expected_components, expected_bits) = vector_data_bits(expected);
    let (actual_components, actual_bits) = vector_data_bits(actual);

    if expected_components != actual_components {
        return Some(format!(
            "component count {expected_components} != {actual_components}"
        ));
    }
    if expected_bits.len() != actual_bits.len() {
        return Some(format!(
            "element count {} != {}",
            expected_bits.len() / expected_components,
            actual_bits.len() / actual_components
        ));
    }
    if let Some(position) = expected_bits
        .iter()
        .zip(actual_bits.iter())
        .position(|(x, y)| x != y)
    {
        return Some(format!(
            "element {} component {} bits {:#010x} != {:#010x}",
            position / expected_components,
            position % expected_components,
            expected_bits[position],
            actual_bits[position]
        ));
    }

    None
}

fn vector_data_bits(data: &VectorData) -> (usize, Vec<u32>) {
    match data {
        VectorData::Vector2(v) => (
            2,
            v.iter()
                .flat_map(|vec| vec.to_array())
                .map(|f| f.to_bits())
                .collect(),
        ),
        VectorData::Vector3(v) => (
            3,
            v.iter()
                .flat_map(|vec| vec.to_array())
                .map(|f| f.to_bits())
                .collect(),
        ),
        VectorData::Vector4(v) => (
            4,
            v.iter()
                .flat_map(|vec| vec.to_array())
                .map(|f| f.to_bits())
                .collect(),
        ),
    }
}

/// Returns the file-level declared buffer sizes and the actual vertex buffer lengths.
pub(crate) fn mesh_buffer_lengths(mesh: &Mesh) -> (Vec<u32>, Vec<usize>) {
    match mesh {
        Mesh::V8(mesh) => inner_buffer_lengths(mesh),
        Mesh::V9(mesh) => inner_buffer_lengths(mesh),
        Mesh::V10(mesh) => inner_buffer_lengths(mesh),
    }
}

fn inner_buffer_lengths<A: Attribute, W: Weight>(mesh: &MeshInner<A, W>) -> (Vec<u32>, Vec<usize>) {
    (
        mesh.buffer_sizes.elements.to_vec(),
        mesh.vertex_buffers
            .elements
            .iter()
            .map(|b| b.elements.len())
            .collect(),
    )
}

/// Returns true if any object attribute reads from the given vertex buffer index.
/// A buffer must be unreferenced before a test treats it as removable.
pub(crate) fn mesh_references_buffer(mesh: &Mesh, buffer_index: u64) -> bool {
    match mesh {
        Mesh::V8(mesh) => inner_references_buffer(mesh, buffer_index),
        Mesh::V9(mesh) => inner_references_buffer(mesh, buffer_index),
        Mesh::V10(mesh) => inner_references_buffer(mesh, buffer_index),
    }
}

fn inner_references_buffer<A: Attribute, W: Weight>(
    mesh: &MeshInner<A, W>,
    buffer_index: u64,
) -> bool {
    mesh.objects.elements.iter().any(|o| {
        o.attributes
            .elements
            .iter()
            .any(|a| a.to_attribute().index == buffer_index)
    })
}

#[cfg(test)]
mod tests {
    use super::super::{AttributeData, BoneInfluence, MeshObjectData, VertexWeight};
    use super::*;

    fn test_object(name: &str) -> MeshObjectData {
        MeshObjectData {
            name: name.to_string(),
            subindex: 0,
            parent_bone_name: "root".to_string(),
            vertex_indices: vec![0, 1, 2],
            positions: vec![AttributeData {
                name: "Position0".to_string(),
                data: VectorData::Vector3(vec![
                    glam::vec3(0.0, 1.0, 2.0),
                    glam::vec3(3.0, 4.0, 5.0),
                    glam::vec3(6.0, 7.0, 8.0),
                ]),
            }],
            texture_coordinates: vec![AttributeData {
                name: "TextureCoordinate0".to_string(),
                data: VectorData::Vector2(vec![
                    glam::vec2(0.0, 0.5),
                    glam::vec2(0.5, 1.0),
                    glam::vec2(1.0, 0.0),
                ]),
            }],
            bone_influences: vec![BoneInfluence {
                bone_name: "bone_a".to_string(),
                vertex_weights: vec![VertexWeight {
                    vertex_index: 1,
                    vertex_weight: 0.75,
                }],
            }],
            ..MeshObjectData::default()
        }
    }

    fn test_mesh_data() -> MeshData {
        MeshData {
            major_version: 1,
            minor_version: 8,
            objects: vec![test_object("a"), test_object("b")],
            is_vs2: true,
        }
    }

    #[test]
    fn equal_meshes_pass() {
        let data = test_mesh_data();
        assert_mesh_data_semantically_equal(&data, &data.clone(), true);
    }

    #[test]
    fn changed_vertex_index_fails_with_object_context() {
        let expected = test_mesh_data();
        let mut actual = expected.clone();
        actual.objects[1].vertex_indices[2] = 0;

        let difference = mesh_data_semantic_difference(&expected, &actual, true).unwrap();
        assert!(difference.contains("object 1 'b'"), "{difference}");
        assert!(difference.contains("vertex index 2"), "{difference}");
    }

    #[test]
    fn changed_attribute_value_fails_with_attribute_context() {
        let expected = test_mesh_data();
        let mut actual = expected.clone();
        match &mut actual.objects[0].positions[0].data {
            VectorData::Vector3(values) => values[1].z = 5.5,
            _ => unreachable!(),
        }

        let difference = mesh_data_semantic_difference(&expected, &actual, true).unwrap();
        assert!(difference.contains("object 0 'a'"), "{difference}");
        assert!(
            difference.contains("positions[0] 'Position0'"),
            "{difference}"
        );
        assert!(difference.contains("element 1 component 2"), "{difference}");
    }

    #[test]
    fn negative_zero_attribute_value_fails_bitwise_comparison() {
        let expected = test_mesh_data();
        let mut actual = expected.clone();
        match &mut actual.objects[0].positions[0].data {
            VectorData::Vector3(values) => values[0].x = -0.0,
            _ => unreachable!(),
        }

        // -0.0 == 0.0 numerically, so this proves the comparison is bitwise.
        assert!(mesh_data_semantic_difference(&expected, &actual, true).is_some());
    }

    #[test]
    fn changed_rigging_influence_fails() {
        let expected = test_mesh_data();
        let mut actual = expected.clone();
        actual.objects[0].bone_influences[0].vertex_weights[0].vertex_weight = 0.5;

        let difference = mesh_data_semantic_difference(&expected, &actual, true).unwrap();
        assert!(
            difference.contains("bone influence [0] 'bone_a'"),
            "{difference}"
        );
    }

    #[test]
    fn object_reordering_fails() {
        let expected = test_mesh_data();
        let mut actual = expected.clone();
        actual.objects.reverse();

        let difference = mesh_data_semantic_difference(&expected, &actual, true).unwrap();
        assert!(difference.contains("name 'a' != 'b'"), "{difference}");
    }

    #[test]
    fn generated_attribute_names_pass_when_names_are_ignored() {
        let expected = test_mesh_data();
        let mut actual = expected.clone();
        actual.objects[0].positions[0].name = "generated".to_string();

        assert!(mesh_data_semantic_difference(&expected, &actual, true).is_some());
        assert!(mesh_data_semantic_difference(&expected, &actual, false).is_none());
    }
}
