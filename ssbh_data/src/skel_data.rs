use glam::Mat4;

// TODO: Include major and minor version?
pub struct SkelData {
    pub bones: Vec<BoneData>,
}

pub struct BoneData {
    pub name: String,
    pub transform: [[f32; 4]; 4],
    pub world_transform: [[f32; 4]; 4],
    pub parent_index: Option<usize>, // TODO: Flags?
}

fn mat4_from_row2d(elements: &[[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array_2d(&elements).transpose()
}

impl SkelData {
    /// Calculates the combined single bind transform matrix, which determines the resting position of a single bound mesh object.
    /// Each bone transform is multiplied by its parents transform recursively starting with `parent_bone_name` until a root node is reached.
    /// Returns the resulting matrix in row major order or `None` if no matrix could be calculated for the given `parent_bone_name`.
    pub fn calculate_single_bind_transform(&self, parent_bone_name: &str) -> Option<[[f32; 4]; 4]> {
        // Attempt to find the parent containing the single bind transform.
        let current_index = self.bones.iter().position(|b| b.name == parent_bone_name)?;

        // Accumulate transforms of a bone with its parent recursively.
        // TODO: There's probably a cleaner way to write this.
        let mut transform = mat4_from_row2d(&self.bones.get(current_index)?.transform);
        let mut parent_index = self.bones[current_index].parent_index;
        while parent_index.is_some() {
            let parent_transform =
                mat4_from_row2d(&self.bones.get(parent_index.unwrap())?.transform);

            transform = transform.mul_mat4(&parent_transform);

            parent_index = self.bones.get(parent_index.unwrap())?.parent_index;
        }

        // Save the result in row-major order.
        Some(transform.transpose().to_cols_array_2d())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_bind_transform_no_parent() {
        let data = SkelData {
            bones: vec![BoneData {
                name: "root".to_string(),
                transform: [[0f32; 4]; 4],
                world_transform: [[0f32; 4]; 4],
                parent_index: None,
            }],
        };

        assert_eq!(None, data.calculate_single_bind_transform("parent"));
    }

    #[test]
    fn single_bind_transform_single_parent() {
        // Use unique values to make sure the matrix is correct.
        let transform = [
            [0f32, 1f32, 2f32, 3f32],
            [4f32, 5f32, 6f32, 7f32],
            [8f32, 9f32, 10f32, 11f32],
            [12f32, 13f32, 14f32, 15f32],
        ];
        let data = SkelData {
            bones: vec![BoneData {
                name: "parent".to_string(),
                transform,
                world_transform: [[0f32; 4]; 4],
                parent_index: None,
            }],
        };

        assert_eq!(
            Some(transform),
            data.calculate_single_bind_transform("parent")
        );
    }

    #[test]
    fn single_bind_transform_multi_parent_chain() {
        // Use non symmetric matrices to check the transpose.
        let data = SkelData {
            bones: vec![
                BoneData {
                    name: "parent".to_string(),
                    transform: [
                        [1f32, 0f32, 0f32, 0f32],
                        [0f32, 2f32, 0f32, 0f32],
                        [0f32, 0f32, 3f32, 1f32],
                        [0f32, 0f32, 0f32, 1f32],
                    ],
                    world_transform: [[0f32; 4]; 4],
                    parent_index: Some(1),
                },
                BoneData {
                    name: "grandparent".to_string(),
                    transform: [
                        [1f32, 0f32, 0f32, 0f32],
                        [0f32, 2f32, 0f32, 0f32],
                        [0f32, 0f32, 3f32, 0f32],
                        [0f32, 0f32, 0f32, 4f32],
                    ],
                    world_transform: [[0f32; 4]; 4],
                    parent_index: None,
                },
            ],
        };

        assert_eq!(
            Some([
                [1f32, 0f32, 0f32, 0f32],
                [0f32, 4f32, 0f32, 0f32],
                [0f32, 0f32, 9f32, 4f32],
                [0f32, 0f32, 0f32, 4f32]
            ]),
            data.calculate_single_bind_transform("parent")
        );
    }
}
