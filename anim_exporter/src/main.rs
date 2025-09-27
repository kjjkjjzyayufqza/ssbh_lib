use clap::Parser;
use glam::Quat;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "anim_exporter")]
#[command(about = "Convert SSBH animation data to Maya anim files")]
struct Args {
    /// Input JSON file path
    #[arg(short, long)]
    input: PathBuf,
    
    /// Output Maya anim file path
    #[arg(short, long)]
    output: PathBuf,
    
    /// Frame rate for the animation (default: 60)
    #[arg(short, long, default_value = "60")]
    fps: f32,
}

#[derive(Debug, Deserialize)]
struct SsbhAnimData {
    major_version: u16,
    minor_version: u16,
    final_frame_index: f32,
    groups: Vec<Group>,
}

#[derive(Debug, Deserialize)]
struct Group {
    group_type: String,
    nodes: Vec<Node>,
}

#[derive(Debug, Deserialize)]
struct Node {
    name: String,
    tracks: Vec<Track>,
}

#[derive(Debug, Deserialize)]
struct Track {
    name: String,
    compensate_scale: bool,
    transform_flags: TransformFlags,
    values: TrackValues,
}

#[derive(Debug, Deserialize)]
struct TransformFlags {
    override_translation: bool,
    override_rotation: bool,
    override_scale: bool,
    override_compensate_scale: bool,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TrackValues {
    Transform { Transform: Vec<Transform> },
}

#[derive(Debug, Deserialize, Clone)]
struct Transform {
    scale: Vector3,
    rotation: Vector4, // Quaternion
    translation: Vector3,
}

#[derive(Debug, Deserialize, Clone)]
struct Vector3 {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(Debug, Deserialize, Clone)]
struct Vector4 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[derive(Debug, Clone)]
struct EulerAngles {
    x: f32, // pitch
    y: f32, // yaw
    z: f32, // roll
}

struct MayaAnimWriter {
    content: String,
    frame_rate: f32,
    final_frame: f32,
}

impl MayaAnimWriter {
    fn new(frame_rate: f32, final_frame: f32) -> Self {
        let mut content = String::new();
        
        // Write Maya anim file header
        content.push_str("animVersion 1.1;\n");
        content.push_str("mayaVersion 2022;\n");
        content.push_str("timeUnit ntscf;\n");
        content.push_str("linearUnit cm;\n");
        content.push_str("angularUnit deg;\n");
        content.push_str(&format!("startTime 1;\n"));
        content.push_str(&format!("endTime {};\n", final_frame as i32 + 1));
        
        Self {
            content,
            frame_rate,
            final_frame,
        }
    }
    
    fn add_property_animation(&mut self, node_name: &str, property: &str, property_name: &str, property_type: &str, values: &[f32], track_index: usize, property_index: usize) {
        self.content.push_str(&format!("anim {} {} {} {} {} {};\n", 
            property, property_name, node_name, 0, track_index, property_index));
        
        self.content.push_str("animData {\n");
        self.content.push_str("  input time;\n");
        
        match property_type {
            "scale" => self.content.push_str("  output unitless;\n"),
            "rotate" => self.content.push_str("  output angular;\n"),
            "translate" => self.content.push_str("  output linear;\n"),
            "visibility" => self.content.push_str("  output unitless;\n"),
            _ => self.content.push_str("  output unitless;\n"),
        }
        
        self.content.push_str("  weighted 0;\n");
        self.content.push_str("  preInfinity constant;\n");
        self.content.push_str("  postInfinity constant;\n");
        self.content.push_str("  keys {\n");
        
        // Add keyframes
        for (frame_index, value) in values.iter().enumerate() {
            let frame_number = frame_index + 1;
            let interpolation = if frame_index == 0 { "fixed fixed" } else { "auto auto" };
            let tangent_data = if frame_index == 0 { " 1 0 0 0 1 0 1" } else { " 1 0 0" };
            
            self.content.push_str(&format!("    {} {} {}{}\n", 
                frame_number, value, interpolation, tangent_data));
        }
        
        self.content.push_str("  }\n");
        self.content.push_str("}\n");
    }
    
    fn get_content(&self) -> &str {
        &self.content
    }
}

// Convert quaternion to Euler angles (in degrees)
fn quaternion_to_euler_degrees(quat: &Vector4) -> EulerAngles {
    let q = Quat::from_xyzw(quat.x, quat.y, quat.z, quat.w);
    let (x, y, z) = q.to_euler(glam::EulerRot::XYZ);
    
    EulerAngles {
        x: x.to_degrees(),
        y: y.to_degrees(),
        z: z.to_degrees(),
    }
}

fn convert_ssbh_to_maya(input_path: &PathBuf, output_path: &PathBuf, fps: f32) -> Result<(), Box<dyn std::error::Error>> {
    // Read and parse the JSON file
    let json_content = fs::read_to_string(input_path)?;
    let anim_data: SsbhAnimData = serde_json::from_str(&json_content)?;
    
    println!("Loaded animation with {} groups, final frame: {}", 
             anim_data.groups.len(), anim_data.final_frame_index);
    
    // Initialize Maya anim writer
    let mut maya_writer = MayaAnimWriter::new(fps, anim_data.final_frame_index);
    
    let mut track_index = 1;
    
    // Process each group and node
    for group in &anim_data.groups {
        if group.group_type == "Transform" {
            for node in &group.nodes {
                println!("Processing node: {}", node.name);
                
                for track in &node.tracks {
                    if track.name == "Transform" {
                        let TrackValues::Transform { Transform: transforms } = &track.values;
                        
                        // Extract scale, rotation, and translation data
                        let scales_x: Vec<f32> = transforms.iter().map(|t| t.scale.x).collect();
                        let scales_y: Vec<f32> = transforms.iter().map(|t| t.scale.y).collect();
                        let scales_z: Vec<f32> = transforms.iter().map(|t| t.scale.z).collect();
                        
                        let translations_x: Vec<f32> = transforms.iter().map(|t| t.translation.x).collect();
                        let translations_y: Vec<f32> = transforms.iter().map(|t| t.translation.y).collect();
                        let translations_z: Vec<f32> = transforms.iter().map(|t| t.translation.z).collect();
                        
                        // Convert quaternions to Euler angles
                        let rotations: Vec<EulerAngles> = transforms.iter()
                            .map(|t| quaternion_to_euler_degrees(&t.rotation))
                            .collect();
                        
                        let rotations_x: Vec<f32> = rotations.iter().map(|r| r.x).collect();
                        let rotations_y: Vec<f32> = rotations.iter().map(|r| r.y).collect();
                        let rotations_z: Vec<f32> = rotations.iter().map(|r| r.z).collect();
                        
                        // Add scale animations
                        maya_writer.add_property_animation(&node.name, "scale.scaleX", "scaleX", "scale", &scales_x, track_index, 0);
                        maya_writer.add_property_animation(&node.name, "scale.scaleY", "scaleY", "scale", &scales_y, track_index, 1);
                        maya_writer.add_property_animation(&node.name, "scale.scaleZ", "scaleZ", "scale", &scales_z, track_index, 2);
                        
                        // Add translation animations
                        maya_writer.add_property_animation(&node.name, "translate.translateX", "translateX", "translate", &translations_x, track_index, 3);
                        maya_writer.add_property_animation(&node.name, "translate.translateY", "translateY", "translate", &translations_y, track_index, 4);
                        maya_writer.add_property_animation(&node.name, "translate.translateZ", "translateZ", "translate", &translations_z, track_index, 5);
                        
                        // Add rotation animations
                        maya_writer.add_property_animation(&node.name, "rotate.rotateX", "rotateX", "rotate", &rotations_x, track_index, 6);
                        maya_writer.add_property_animation(&node.name, "rotate.rotateY", "rotateY", "rotate", &rotations_y, track_index, 7);
                        maya_writer.add_property_animation(&node.name, "rotate.rotateZ", "rotateZ", "rotate", &rotations_z, track_index, 8);
                        
                        // Add visibility animation (always 1 for now)
                        let visibility: Vec<f32> = vec![1.0; transforms.len()];
                        maya_writer.add_property_animation(&node.name, "visibility", "visibility", "visibility", &visibility, track_index, 9);
                        
                        track_index += 1;
                    }
                }
            }
        }
    }
    
    // Write the Maya anim file
    fs::write(output_path, maya_writer.get_content())?;
    println!("Successfully exported Maya anim file to: {}", output_path.display());
    
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    if !args.input.exists() {
        eprintln!("Error: Input file does not exist: {}", args.input.display());
        std::process::exit(1);
    }
    
    convert_ssbh_to_maya(&args.input, &args.output, args.fps)?;
    
    Ok(())
}