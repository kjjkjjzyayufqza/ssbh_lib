use ssbh_lib::prelude::*;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let anim = Anim::from_file("example.nuanmb")?;
    
    match anim {
        Anim::V12 { tracks, buffers, .. } => {
            println!("Animation version 1.2");
            println!("Total tracks: {}", tracks.elements.len());
            println!("Total buffers: {}", buffers.elements.len());
            
            for (track_idx, track) in tracks.elements.iter().enumerate() {
                let track_name = track.name.to_string_lossy();
                if track_name.contains("MOMO") {
                    println!("\n=== Track #{}: {} ===", track_idx, track_name);
                    println!("Track type: {:?}", track.track_type);
                    println!("Properties count: {}", track.properties.elements.len());
                    
                    for (prop_idx, property) in track.properties.elements.iter().enumerate() {
                        let prop_name = property.name.to_string_lossy();
                        let buffer_idx = property.buffer_index as usize;
                        
                        println!("\n  Property #{}: {}", prop_idx, prop_name);
                        println!("  Buffer index: {}", buffer_idx);
                        
                        if buffer_idx < buffers.elements.len() {
                            let buffer = &buffers.elements[buffer_idx];
                            let data = &buffer.elements;
                            println!("  Buffer size: {} bytes", data.len());
                            
                            // Print all bytes as hex
                            print!("  Data (hex): ");
                            for (i, byte) in data.iter().enumerate() {
                                if i % 16 == 0 && i > 0 {
                                    print!("\n              ");
                                }
                                print!("{:02X} ", byte);
                            }
                            println!();
                            
                            // Try to interpret as header + data
                            if data.len() >= 4 {
                                let header = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                                println!("  Header: 0x{:04X}", header);
                                
                                // If it looks like it contains floats
                                if data.len() >= 16 {
                                    println!("  Interpreting as floats:");
                                    for i in (4..std::cmp::min(data.len(), 40)).step_by(4) {
                                        if i + 4 <= data.len() {
                                            let f = f32::from_le_bytes([data[i], data[i+1], data[i+2], data[i+3]]);
                                            println!("    Offset {}: {}", i, f);
                                        }
                                    }
                                }
                            }
                        } else {
                            println!("  ERROR: Buffer index out of range!");
                        }
                    }
                }
            }
        }
        _ => println!("Not a version 1.2 animation"),
    }
    
    Ok(())
}
