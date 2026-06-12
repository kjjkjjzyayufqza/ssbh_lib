//! Tests for [MeshWriteProfile](super::MeshWriteProfile) serialization policies.
//!
//! The legacy profile must preserve the established writer output.
//! The EXVS2 canonical profile must match the compact reconstruction verified
//! against StudioSB output while preserving all model semantics.
use super::semantic_compare::{
    assert_mesh_data_semantically_equal, mesh_buffer_lengths, mesh_data_semantic_difference,
    mesh_references_buffer,
};
use super::vector_data::{VectorDataV8, VersionedVectorData};
use super::*;
use mesh_attributes::MeshAttributes;

fn write_mesh_bytes(mesh: &Mesh) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    mesh.write(&mut cursor).unwrap();
    cursor.into_inner()
}

fn reparse_mesh_data(bytes: &[u8]) -> MeshData {
    let mut cursor = Cursor::new(bytes);
    MeshData::read(&mut cursor).unwrap()
}

fn vector3_values(count: usize, offset: f32) -> Vec<[f32; 3]> {
    (0..count)
        .map(|i| {
            let base = offset + i as f32;
            [base, base + 0.25, base + 0.5]
        })
        .collect()
}

fn vector2_values(count: usize, offset: f32) -> Vec<[f32; 2]> {
    (0..count)
        .map(|i| {
            let base = offset + i as f32;
            [base, base + 0.125]
        })
        .collect()
}

fn vector4_values(count: usize, offset: f32) -> Vec<[f32; 4]> {
    (0..count)
        .map(|i| {
            let base = offset + i as f32;
            [base, base + 0.25, base + 0.5, base + 0.75]
        })
        .collect()
}

/// Builds a mesh object with the layout the application produces for EXVS2:
/// position, normal, two binormals, two tangents, one UV set, the generated
/// HalfFloat2 set, and two color sets.
fn exvs2_object(name: &str, vertex_count: usize) -> MeshObjectData {
    assert_eq!(0, vertex_count % 3, "fixture indices cover every vertex");
    MeshObjectData {
        name: name.to_string(),
        subindex: 0,
        parent_bone_name: "root".to_string(),
        vertex_indices: (0..vertex_count as u32).collect(),
        positions: vec![AttributeData {
            name: String::new(),
            data: VectorData::Vector3(vector3_values(vertex_count, 0.0)),
        }],
        normals: vec![AttributeData {
            name: String::new(),
            data: VectorData::Vector3(vector3_values(vertex_count, 100.0)),
        }],
        binormals: vec![
            AttributeData {
                name: String::new(),
                data: VectorData::Vector3(vector3_values(vertex_count, 200.0)),
            },
            AttributeData {
                name: String::new(),
                data: VectorData::Vector3(vector3_values(vertex_count, 300.0)),
            },
        ],
        tangents: vec![
            AttributeData {
                name: String::new(),
                data: VectorData::Vector3(vector3_values(vertex_count, 400.0)),
            },
            AttributeData {
                name: String::new(),
                data: VectorData::Vector3(vector3_values(vertex_count, 500.0)),
            },
        ],
        texture_coordinates: vec![
            AttributeData {
                name: String::new(),
                data: VectorData::Vector2(vector2_values(vertex_count, 600.0)),
            },
            AttributeData {
                name: "HalfFloat2_0".to_string(),
                data: VectorData::Vector4(vector4_values(vertex_count, 700.0)),
            },
        ],
        color_sets: vec![
            AttributeData {
                name: "colorSet0".to_string(),
                data: VectorData::Vector2(vector2_values(vertex_count, 800.0)),
            },
            AttributeData {
                name: "colorSet1".to_string(),
                data: VectorData::Vector2(vector2_values(vertex_count, 900.0)),
            },
        ],
        bone_influences: vec![BoneInfluence {
            bone_name: format!("{name}_bone"),
            vertex_weights: vec![
                VertexWeight {
                    vertex_index: 0,
                    vertex_weight: 0.75,
                },
                VertexWeight {
                    vertex_index: vertex_count as u32 - 1,
                    vertex_weight: 0.25,
                },
            ],
        }],
        ..MeshObjectData::default()
    }
}

fn exvs2_mesh_data(minor_version: u16, vertex_counts: &[usize]) -> MeshData {
    MeshData {
        major_version: 1,
        minor_version,
        objects: vertex_counts
            .iter()
            .enumerate()
            .map(|(i, count)| exvs2_object(&format!("object{i}"), *count))
            .collect(),
        is_vs2: true,
    }
}

fn mesh_v8_inner(mesh: &Mesh) -> &MeshInner<AttributeV8, SsbhArray<VertexWeightV8>> {
    match mesh {
        Mesh::V8(inner) => inner,
        _ => panic!("expected a v1.8 mesh"),
    }
}

#[test]
fn legacy_profile_matches_try_from_bytes() {
    for minor_version in [8, 9, 10] {
        let data = exvs2_mesh_data(minor_version, &[12, 6]);

        let from_try_from = write_mesh_bytes(&Mesh::try_from(&data).unwrap());
        let from_profile = write_mesh_bytes(
            &data
                .to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
                .unwrap(),
        );

        assert_eq!(
            from_try_from, from_profile,
            "legacy profile changed v1.{minor_version} output"
        );
    }
}

#[test]
fn profile_conversion_does_not_mutate_input() {
    let data = exvs2_mesh_data(8, &[12]);
    let original = data.clone();

    data.to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
        .unwrap();
    data.to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
        .unwrap();

    assert_mesh_data_semantically_equal(&original, &data, true);
}

#[test]
fn v8_legacy_profile_writes_dummy_buffer2() {
    let data = exvs2_mesh_data(8, &[12, 6]);
    let mesh = data
        .to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
        .unwrap();

    let (declared, actual) = mesh_buffer_lengths(&mesh);
    assert_eq!(32 * 18, declared[2]);
    assert_eq!(32 * 18, actual[2] as u32);

    let inner = mesh_v8_inner(&mesh);
    assert_eq!(32, inner.objects.elements[0].stride2);
    assert_eq!(0, inner.objects.elements[0].vertex_buffer2_offset);
    assert_eq!(32, inner.objects.elements[1].stride2);
    assert_eq!(32 * 12, inner.objects.elements[1].vertex_buffer2_offset);
}

#[test]
fn v8_canonical_profile_omits_dummy_buffer2() {
    let data = exvs2_mesh_data(8, &[12, 6]);

    let legacy = data
        .to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
        .unwrap();
    let canonical = data
        .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
        .unwrap();

    // The dummy buffer must be unreferenced before it can be removed.
    assert!(!mesh_references_buffer(&legacy, 2));
    assert!(!mesh_references_buffer(&canonical, 2));

    let (declared, actual) = mesh_buffer_lengths(&canonical);
    assert_eq!(0, declared[2]);
    assert_eq!(0, actual[2]);

    let legacy_inner = mesh_v8_inner(&legacy);
    let canonical_inner = mesh_v8_inner(&canonical);

    // Verified against StudioSB canonical output: stride2 stays 32 and
    // vertex_buffer2_offset mirrors vertex_buffer1_offset.
    for object in &canonical_inner.objects.elements {
        assert_eq!(32, object.stride2);
        assert_eq!(object.vertex_buffer1_offset, object.vertex_buffer2_offset);
    }

    // Buffer 0, buffer 1, vertex indices, and rigging are unchanged.
    assert_eq!(
        legacy_inner.vertex_buffers.elements[0],
        canonical_inner.vertex_buffers.elements[0]
    );
    assert_eq!(
        legacy_inner.vertex_buffers.elements[1],
        canonical_inner.vertex_buffers.elements[1]
    );
    assert_eq!(legacy_inner.index_buffer, canonical_inner.index_buffer);
    assert_eq!(
        legacy_inner.rigging_buffers.elements,
        canonical_inner.rigging_buffers.elements
    );
    assert_eq!(legacy_inner.bounding_info, canonical_inner.bounding_info);
    assert_eq!(
        legacy_inner.polygon_index_size,
        canonical_inner.polygon_index_size
    );
}

#[test]
fn v9_profiles_handle_dummy_buffer2() {
    let data = exvs2_mesh_data(9, &[12, 6]);

    let legacy = data
        .to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
        .unwrap();
    let canonical = data
        .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
        .unwrap();

    let (legacy_declared, legacy_actual) = mesh_buffer_lengths(&legacy);
    assert_eq!(32 * 18, legacy_declared[2]);
    assert_eq!(32 * 18, legacy_actual[2] as u32);

    assert!(!mesh_references_buffer(&canonical, 2));
    let (canonical_declared, canonical_actual) = mesh_buffer_lengths(&canonical);
    assert_eq!(0, canonical_declared[2]);
    assert_eq!(0, canonical_actual[2]);
}

#[test]
fn v10_output_identical_for_both_profiles() {
    let data = exvs2_mesh_data(10, &[12, 6]);

    let legacy = write_mesh_bytes(
        &data
            .to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
            .unwrap(),
    );
    let canonical = write_mesh_bytes(
        &data
            .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
            .unwrap(),
    );

    assert_eq!(legacy, canonical);
}

#[test]
fn v8_canonical_output_reparses_semantically_equal() {
    let data = exvs2_mesh_data(8, &[12, 6]);

    let legacy_bytes = write_mesh_bytes(
        &data
            .to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
            .unwrap(),
    );
    let canonical_bytes = write_mesh_bytes(
        &data
            .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
            .unwrap(),
    );

    let legacy_reparsed = reparse_mesh_data(&legacy_bytes);
    let canonical_reparsed = reparse_mesh_data(&canonical_bytes);

    // Both reparsed files use generated v1.8 attribute names, so compare them strictly.
    assert_mesh_data_semantically_equal(&legacy_reparsed, &canonical_reparsed, true);

    // Mesh v1.8 does not store attribute names, so ignore names against the pre-write data.
    assert_mesh_data_semantically_equal(&data, &canonical_reparsed, false);
}

#[test]
fn v8_canonical_acceptance_828_vertices() {
    // Mirrors the measured anti_L sample: three objects totaling 828 vertices
    // must save exactly 828 * 32 = 26,496 bytes of dummy buffer2 data.
    let data = exvs2_mesh_data(8, &[402, 24, 402]);

    let legacy_bytes = write_mesh_bytes(
        &data
            .to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
            .unwrap(),
    );
    let canonical_bytes = write_mesh_bytes(
        &data
            .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
            .unwrap(),
    );

    assert_eq!(26_496, legacy_bytes.len() - canonical_bytes.len());

    let canonical_reparsed = reparse_mesh_data(&canonical_bytes);
    assert_mesh_data_semantically_equal(&data, &canonical_reparsed, false);
}

#[test]
fn index_width_16_bit_boundary() {
    // The maximum index 65,535 still fits in two bytes.
    let vertex_count = u16::MAX as usize + 1;
    let mut object = MeshObjectData {
        name: "boundary16".to_string(),
        vertex_indices: vec![0, 1, u16::MAX as u32],
        positions: vec![AttributeData {
            name: String::new(),
            data: VectorData::Vector3(vector3_values(vertex_count, 0.0)),
        }],
        ..MeshObjectData::default()
    };
    object.subindex = 0;

    let data = MeshData {
        major_version: 1,
        minor_version: 8,
        objects: vec![object],
        is_vs2: true,
    };

    for profile in [
        MeshWriteProfile::LegacyCompatible,
        MeshWriteProfile::Vs2Canonical,
    ] {
        let mesh = data.to_mesh_with_profile(profile).unwrap();
        let inner = mesh_v8_inner(&mesh);
        assert_eq!(
            DrawElementType::UnsignedShort,
            inner.objects.elements[0].draw_element_type
        );
        assert_eq!(2 * 3, inner.index_buffer.elements.len());

        let reparsed = reparse_mesh_data(&write_mesh_bytes(&mesh));
        assert_eq!(
            vec![0, 1, u16::MAX as u32],
            reparsed.objects[0].vertex_indices
        );
    }
}

#[test]
fn index_width_32_bit_boundary() {
    // The maximum index 65,536 requires four bytes per index.
    let vertex_count = u16::MAX as usize + 2;
    let object = MeshObjectData {
        name: "boundary32".to_string(),
        vertex_indices: vec![0, 1, u16::MAX as u32 + 1],
        positions: vec![AttributeData {
            name: String::new(),
            data: VectorData::Vector3(vector3_values(vertex_count, 0.0)),
        }],
        ..MeshObjectData::default()
    };

    let data = MeshData {
        major_version: 1,
        minor_version: 8,
        objects: vec![object],
        is_vs2: true,
    };

    for profile in [
        MeshWriteProfile::LegacyCompatible,
        MeshWriteProfile::Vs2Canonical,
    ] {
        let mesh = data.to_mesh_with_profile(profile).unwrap();
        let inner = mesh_v8_inner(&mesh);
        assert_eq!(
            DrawElementType::UnsignedInt,
            inner.objects.elements[0].draw_element_type
        );
        assert_eq!(4 * 3, inner.index_buffer.elements.len());

        let reparsed = reparse_mesh_data(&write_mesh_bytes(&mesh));
        assert_eq!(
            vec![0, 1, u16::MAX as u32 + 1],
            reparsed.objects[0].vertex_indices
        );
    }
}

#[test]
fn out_of_range_index_error_includes_object_name() {
    let object = MeshObjectData {
        name: "broken".to_string(),
        vertex_indices: vec![0, 1, 3],
        positions: vec![AttributeData {
            name: String::new(),
            data: VectorData::Vector3(vector3_values(3, 0.0)),
        }],
        ..MeshObjectData::default()
    };

    let data = MeshData {
        major_version: 1,
        minor_version: 8,
        objects: vec![object],
        is_vs2: true,
    };

    let result = data.to_mesh_with_profile(MeshWriteProfile::Vs2Canonical);
    match result {
        Err(error::Error::VertexIndexOutOfRange {
            mesh_object_name,
            vertex_index,
            vertex_count,
        }) => {
            assert_eq!("broken", mesh_object_name);
            assert_eq!(3, vertex_index);
            assert_eq!(3, vertex_count);
        }
        other => panic!("expected VertexIndexOutOfRange, got {other:?}"),
    }
}

#[test]
fn small_meshes_remain_16_bit() {
    let data = exvs2_mesh_data(8, &[12, 6]);

    for profile in [
        MeshWriteProfile::LegacyCompatible,
        MeshWriteProfile::Vs2Canonical,
    ] {
        let mesh = data.to_mesh_with_profile(profile).unwrap();
        let inner = mesh_v8_inner(&mesh);
        for object in &inner.objects.elements {
            assert_eq!(DrawElementType::UnsignedShort, object.draw_element_type);
        }
        assert_eq!(2 * 18, inner.index_buffer.elements.len());
    }
}

#[test]
fn canonical_profile_errors_if_attribute_references_buffer2() {
    // No current attribute layout assigns buffer 2. If a future layout does,
    // the canonical profile must fail loudly instead of dropping the data.
    let object = MeshObjectData {
        name: "future_layout".to_string(),
        vertex_indices: vec![0, 1, 2],
        positions: vec![AttributeData {
            name: String::new(),
            data: VectorData::Vector3(vector3_values(3, 0.0)),
        }],
        ..MeshObjectData::default()
    };

    let result = create_mesh_object(
        &object,
        &mut [
            &mut Cursor::new(Vec::new()),
            &mut Cursor::new(Vec::new()),
            &mut Cursor::new(Vec::new()),
            &mut Cursor::new(Vec::new()),
        ],
        &mut 0,
        &mut Cursor::new(Vec::new()),
        MeshWriteProfile::Vs2Canonical,
        |_| MeshAttributes {
            buffer_info: [
                (12, VersionedVectorData::V8(Vec::new())),
                (0, VersionedVectorData::V8(Vec::new())),
                (
                    32,
                    VersionedVectorData::V8(vec![VectorDataV8::Float3(vector3_values(3, 0.0))]),
                ),
                (0, VersionedVectorData::V8(Vec::new())),
            ],
            attributes: vec![AttributeV8 {
                usage: AttributeUsageV8::Position,
                data_type: AttributeDataTypeV8::Float3,
                buffer_index: 2,
                buffer_offset: 0,
                subindex: 0,
            }]
            .into(),
            use_buffer2: true,
        },
    );

    match result {
        Err(error::Error::AttributeReferencesOmittedBuffer {
            mesh_object_name,
            buffer_index,
        }) => {
            assert_eq!("future_layout", mesh_object_name);
            assert_eq!(2, buffer_index);
        }
        other => panic!("expected AttributeReferencesOmittedBuffer, got {other:?}"),
    }
}

#[test]
fn exvs2_attribute_layout_locked() {
    let data = exvs2_mesh_data(8, &[12]);

    let legacy = data
        .to_mesh_with_profile(MeshWriteProfile::LegacyCompatible)
        .unwrap();
    let canonical = data
        .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
        .unwrap();

    let legacy_object = &mesh_v8_inner(&legacy).objects.elements[0];
    let canonical_object = &mesh_v8_inner(&canonical).objects.elements[0];

    // The canonical profile keeps the attribute metadata identical to legacy.
    assert_eq!(legacy_object.attributes, canonical_object.attributes);
    assert_eq!(72, canonical_object.stride0);
    assert_eq!(40, canonical_object.stride1);

    let expected = [
        (
            AttributeUsageV8::Position,
            AttributeDataTypeV8::Float3,
            0,
            0,
            0,
        ),
        (
            AttributeUsageV8::Normal,
            AttributeDataTypeV8::Float3,
            0,
            12,
            0,
        ),
        (
            AttributeUsageV8::Binormal,
            AttributeDataTypeV8::Float3,
            0,
            24,
            0,
        ),
        (
            AttributeUsageV8::Tangent,
            AttributeDataTypeV8::Float3,
            0,
            36,
            0,
        ),
        (
            AttributeUsageV8::Binormal,
            AttributeDataTypeV8::Float3,
            0,
            48,
            1,
        ),
        (
            AttributeUsageV8::Tangent,
            AttributeDataTypeV8::Float3,
            0,
            60,
            1,
        ),
        (
            AttributeUsageV8::TextureCoordinate,
            AttributeDataTypeV8::Float2,
            1,
            0,
            0,
        ),
        (
            AttributeUsageV8::ColorSet,
            AttributeDataTypeV8::Float2,
            1,
            8,
            0,
        ),
        (
            AttributeUsageV8::ColorSet,
            AttributeDataTypeV8::Float2,
            1,
            16,
            1,
        ),
        (
            AttributeUsageV8::HalfFloat2,
            AttributeDataTypeV8::Float4,
            1,
            24,
            0,
        ),
    ];

    assert_eq!(expected.len(), canonical_object.attributes.elements.len());
    for (attribute, (usage, data_type, buffer_index, buffer_offset, subindex)) in
        canonical_object.attributes.elements.iter().zip(expected)
    {
        assert_eq!(
            &AttributeV8 {
                usage,
                data_type,
                buffer_index,
                buffer_offset,
                subindex,
            },
            attribute
        );
    }
}

#[test]
fn zero_valued_attributes_are_preserved() {
    let mut data = exvs2_mesh_data(8, &[12]);
    for color_set in &mut data.objects[0].color_sets {
        color_set.data = VectorData::Vector2(vec![[0.0, 0.0]; 12]);
    }

    let canonical = data
        .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
        .unwrap();
    let object = &mesh_v8_inner(&canonical).objects.elements[0];

    // The profile only removes the unreferenced dummy buffer.
    // All-zero attribute data is still semantic data.
    assert_eq!(10, object.attributes.elements.len());

    let reparsed = reparse_mesh_data(&write_mesh_bytes(&canonical));
    assert_mesh_data_semantically_equal(&data, &reparsed, false);
}

#[test]
fn object_names_and_subindices_survive_round_trip() {
    let data = exvs2_mesh_data(8, &[12, 6, 9]);

    let canonical_bytes = write_mesh_bytes(
        &data
            .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
            .unwrap(),
    );
    let reparsed = reparse_mesh_data(&canonical_bytes);

    assert_eq!(data.objects.len(), reparsed.objects.len());
    for (expected, actual) in data.objects.iter().zip(reparsed.objects.iter()) {
        assert_eq!(expected.name, actual.name);
        // The VS2 writer normalizes subindices to 0; the fixtures use 0 already.
        assert_eq!(expected.subindex, actual.subindex);
        assert_eq!(expected.parent_bone_name, actual.parent_bone_name);
    }
}

#[test]
fn semantic_difference_is_detected_after_round_trip() {
    // Guard against the comparator passing because reparsing loses data.
    let data = exvs2_mesh_data(8, &[12]);

    let canonical_bytes = write_mesh_bytes(
        &data
            .to_mesh_with_profile(MeshWriteProfile::Vs2Canonical)
            .unwrap(),
    );
    let reparsed = reparse_mesh_data(&canonical_bytes);

    let mut modified = data.clone();
    match &mut modified.objects[0].positions[0].data {
        VectorData::Vector3(values) => values[3][1] += 1.0,
        _ => unreachable!(),
    }

    assert!(mesh_data_semantic_difference(&modified, &reparsed, false).is_some());
}
