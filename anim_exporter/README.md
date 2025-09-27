# SSBH to Maya Animation Converter

This tool converts SSBH animation data (in JSON format) to Maya `.anim` files.

## Features

- Converts SSBH transform data to Maya keyframe animation
- Supports scale, rotation, and translation channels
- Converts quaternion rotations to Euler angles (degrees)
- Generates proper Maya `.anim` file format
- Configurable frame rate (default: 60 fps)

## Usage

```bash
cargo run -- -i <input.json> -o <output.anim> [-f <fps>]
```

### Arguments

- `-i, --input <PATH>`: Input JSON file containing SSBH animation data
- `-o, --output <PATH>`: Output Maya anim file path
- `-f, --fps <FPS>`: Frame rate for the animation (default: 60)

### Example

```bash
# Convert example animation data
cargo run -- -i ../example.nuanmb.json -o converted_animation.anim

# Convert with custom frame rate
cargo run -- -i ../example.nuanmb.json -o converted_animation.anim -f 30
```

## Input Format

The tool expects SSBH animation data in JSON format with the following structure:

```json
{
  "major_version": 1,
  "minor_version": 2,
  "final_frame_index": 60.0,
  "groups": [
    {
      "group_type": "Transform",
      "nodes": [
        {
          "name": "BONE_NAME",
          "tracks": [
            {
              "name": "Transform",
              "values": {
                "Transform": [
                  {
                    "scale": {"x": 1.0, "y": 1.0, "z": 1.0},
                    "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                    "translation": {"x": 0.0, "y": 0.0, "z": 0.0}
                  }
                ]
              }
            }
          ]
        }
      ]
    }
  ]
}
```

## Output Format

The tool generates Maya `.anim` files with the following format:

- File header with version information
- Separate animation curves for each transform component:
  - `scale.scaleX`, `scale.scaleY`, `scale.scaleZ`
  - `translate.translateX`, `translate.translateY`, `translate.translateZ`
  - `rotate.rotateX`, `rotate.rotateY`, `rotate.rotateZ`
  - `visibility` (always 1.0)

## Technical Details

### Quaternion to Euler Conversion

The tool converts quaternion rotations (XYZW) to Euler angles in XYZ order, with angles output in degrees as required by Maya.

### Keyframe Generation

- First keyframe uses `fixed fixed` interpolation
- Subsequent keyframes use `auto auto` interpolation
- Proper tangent data is included for smooth animation

## Dependencies

- `clap`: Command-line argument parsing
- `serde`: JSON serialization/deserialization
- `glam`: Mathematics library for quaternion conversions
- `ssbh_data`: SSBH format support (local dependency)

## Build

```bash
cargo build --release
```

## Testing

The tool has been tested with the included `example.nuanmb.json` file, which contains animation data for 24 bone nodes across 61 frames.
