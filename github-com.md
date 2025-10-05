
### descatal commented on Dec 31, 2022on Dec 31, 2022

[![@descatal](https://avatars.githubusercontent.com/u/54780311?u=d94664f81c929a793cc525730c2bbec90568e778&v=4&size=48)](https://github.com/descatal)

[descatal](https://github.com/descatal)

[on Dec 31, 2022on Dec 31, 2022](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1368311765) · edited by [descatal](https://github.com/descatal)

Edits

More actions

Hello SMG, are there any progress made on this? I have a game called Gundam Versus (PS4, 2017) that utilizes version 1.2 .nuanmb for every animation.

I've tried working by my own by looking at the c# code on how compressed buffers are parsed, but made no headway, since the way they store the bit count and stuff is quite different. May I ask are there any documentation regarding how the v1.2 parse the compressed bytes? If you'd like I could post the JSON files here.

Edit: I also forgot to mention that I did decompile the function that parse the compressed bytes in IDA, but I have no idea what it is trying to do, if you need some reference on that let me know.

[![ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=80)](https://github.com/ScanMountGoat)

### ScanMountGoat commented on Dec 31, 2022on Dec 31, 2022

[![@ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=48)](https://github.com/ScanMountGoat)

[ScanMountGoat](https://github.com/ScanMountGoat)

[on Dec 31, 2022on Dec 31, 2022](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1368315647) · edited by [ScanMountGoat](https://github.com/ScanMountGoat)

Edits

MemberAuthor

More actions

I don't have many examples of version 1.2 animations, so I haven't looked into it much. The only examples I have are uncompressed. I currently only have compression implemented for version 2.0 and 2.1. I'm not aware of any C# implementations that correctly implement anim compression for any version. The primary goal is to support tooling and applications for Smash Ultimate, but I'm open to supporting any other games that use the SSBH/HBSS format.

Compression Headers (2.0+)

[ssbh\_lib/ssbh\_data/src/anim\_data/compression.rs](https://github.com/ultimate-research/ssbh_lib/blob/6d03d979b6a5bfa03564c8f609057fdcc25eddbd/ssbh_data/src/anim_data/compression.rs#L29-L44)

Lines 29 to 44
in
[6d03d97](https://github.com/ultimate-research/ssbh_lib/commit/6d03d979b6a5bfa03564c8f609057fdcc25eddbd)

|     |     |
| --- | --- |
|  | #\[derive(Debug,BinRead,SsbhWrite)\] |
|  | pubstructCompressedTrackData<T:CompressedData>{ |
|  | pubheader:CompressedHeader<T>, |
|  | pubcompression:T::Compression, |
|  | } |
|  |  |
|  | // TODO: These should be non nullable pointers? |
|  | #\[derive(Debug,BinRead,SsbhWrite)\] |
|  | pubstructCompressedHeader<T:CompressedData>{ |
|  | pubunk\_4:u16,// TODO: Always 4? |
|  | pubflags:CompressionFlags,// TODO: These are used for texture transforms as well? |
|  | pubdefault\_data:Ptr16<T>, |
|  | pubbits\_per\_entry:u16, |
|  | pubcompressed\_data:Ptr32<CompressedBuffer>, |
|  | pubframe\_count:u32, |
|  | } |

Example Compressed Buffer (2.0+)

[ssbh\_lib/ssbh\_data/src/anim\_data/buffers.rs](https://github.com/ultimate-research/ssbh_lib/blob/6d03d979b6a5bfa03564c8f609057fdcc25eddbd/ssbh_data/src/anim_data/buffers.rs#L1651-L1705)

Lines 1651 to 1705
in
[6d03d97](https://github.com/ultimate-research/ssbh_lib/commit/6d03d979b6a5bfa03564c8f609057fdcc25eddbd)

|     |     |
| --- | --- |
|  | #\[test\] |
|  | fnread\_compressed\_transform\_multiple\_frames\_const\_uniform\_scale(){ |
|  | // assist/shovelknight/model/body/c00/model.nuanmb, ArmL, Transform |
|  | let data = hex!( |
|  | // header |
|  | 04000600 a0002b00 cc000000 02000000 |
|  | // scale compression |
|  | 0000803f 0000803f 1000000000000000 |
|  | 0000803f 0000803f 1000000000000000 |
|  | 0000803f 0000803f 1000000000000000 |
|  | // rotation compression |
|  | 00000000 b9bc433d 0d000000 00000000 |
|  | e27186bd 000000000d000000 00000000 |
|  | 00000000 ada2273f 1000000000000000 |
|  | // translation compression |
|  | 16a41d40 16a41d40 1000000000000000 |
|  | 00000000000000001000000000000000 |
|  | 00000000000000001000000000000000 |
|  | // default value |
|  | 0000803f 0000803f 0000803f |
|  | 0000000000000000000000000000803f |
|  | 16a41d40 0000000000000000 |
|  | 00000000 |
|  | // compressed values |
|  | 00e0ff03 00f8ff00 e0ff1f |
|  | ); |
|  |  |
|  | let(values, compensate\_scale) = read\_track\_values( |
|  | &data, |
|  | TrackFlags{ |
|  | track\_type:TrackTypeV2::Transform, |
|  | compression\_type:CompressionType::Compressed, |
|  | }, |
|  | 2, |
|  | ) |
|  | .unwrap(); |
|  |  |
|  | assert\_eq!(false, compensate\_scale); |
|  |  |
|  | assert!(matches!(values, |
|  | TrackValues::Transform(values) |
|  | if values == vec!\[ |\
|  | Transform{ |\
|  | translation:Vector3::new(2.46314,0.0,0.0), |\
|  | rotation:Vector4::new(0.0,0.0,0.0,1.0), |\
|  | scale:Vector3::new(1.0,1.0,1.0), |\
|  | }, |\
|  | Transform{ |\
|  | translation:Vector3::new(2.46314,0.0,0.0), |\
|  | rotation:Vector4::new(0.0477874, -0.0656469,0.654826,0.7514052), |\
|  | scale:Vector3::new(1.0,1.0,1.0), |\
|  | } |\
|  | \] |
|  | )); |
|  | } |

[![ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=80)](https://github.com/ScanMountGoat)

### ScanMountGoat commented on Dec 31, 2022on Dec 31, 2022

[![@ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=48)](https://github.com/ScanMountGoat)

[ScanMountGoat](https://github.com/ScanMountGoat)

[on Dec 31, 2022on Dec 31, 2022](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1368316218) · edited by [ScanMountGoat](https://github.com/ScanMountGoat)

Edits

MemberAuthor

More actions

> Edit: I also forgot to mention that I did decompile the function that parse the compressed bytes in IDA, but I have no idea what it is trying to do, if you need some reference on that let me know.

I'm not sure if reverse engineering the decompiled code is any easier than reverse engineering the binary files themselves. Let me know if you find anything interesting. The anim version 1.2 code is partially implemented in ssbh\_data already except for support for compression.

[![descatal](https://avatars.githubusercontent.com/u/54780311?u=d94664f81c929a793cc525730c2bbec90568e778&v=4&size=80)](https://github.com/descatal)

### descatal commented on Dec 31, 2022on Dec 31, 2022

[![@descatal](https://avatars.githubusercontent.com/u/54780311?u=d94664f81c929a793cc525730c2bbec90568e778&v=4&size=48)](https://github.com/descatal)

[descatal](https://github.com/descatal)

[on Dec 31, 2022on Dec 31, 2022](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1368339249)

More actions

Thanks for the reply, I'll just provide an example of the v1.2 nuanmb file.

So here's my finding:

Please note that in this game most of the animation are done on Rotation except for the main control and base bones, and scale is nonexistent since it is mostly controlled by the script file (good old msc).

Also I have the .anim counterpart that was extracted from the older game (it's the same animation) for cross referencing.

[![image](https://user-images.githubusercontent.com/54780311/210159855-b00383c1-eac7-41bc-ade7-db474a4560ee.png)](https://user-images.githubusercontent.com/54780311/210159855-b00383c1-eac7-41bc-ade7-db474a4560ee.png)

[![image](https://user-images.githubusercontent.com/54780311/210160039-ccf20aaf-2d5e-447b-b35c-28beecbd0190.png)](https://user-images.githubusercontent.com/54780311/210160039-ccf20aaf-2d5e-447b-b35c-28beecbd0190.png)

I'll just do a header breakdown here:

0x00 - 0x04 - Flags, I'll delve into 0x0934 and 0x0944 for the most part since these two contains most of the data.

0x04 - 0x08 - Frame count (e.g. 40 frame)

0x08 - 0x0C - Unknown Float 1, always 1

0x0C - 0x10 - Unknown Float 2

0x10 - 0x12 - Unknown Short, always 2

0x12 - 0x14 - Unknown Short, I am suspecting this is the bitcount that's used in decompression

For 0x0934:

0x14 - 0x20 - XYZ keys for translation in Float, at the first key (e.g. 0)

0x20 - 0x2C - XYZ keys for translation in Float, at some random key between first and final (e.g. 34)

0x2C - 0x38 - XYZ keys for translation in Float, at the final key (e.g. 40)

0x38 - End - The compressed data

For 0x0944:

0x14 - 0x44 - 12 different Float that's related to rotation, could be radian as the range is always between 0 - 2.

0x44 - End - The compressed data

I am not entirely sure how do I apply the same decompression logic that we have in 2.0, since there's a lot of information missing that 2.0 decompression relies on (e.g. bit count, start and end for lerp function, default values etc.)

I have added you on Discord if you don't mind me disturbing you there.

Thank you for your time, and just let me know if you have any insight on these based on your expertise dealing with other versions.

[Front.zip](https://github.com/ultimate-research/ssbh_lib/files/10328209/Front.zip)

[![ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=80)](https://github.com/ScanMountGoat)

### ScanMountGoat commented on Jan 1, 2023on Jan 1, 2023

[![@ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=48)](https://github.com/ScanMountGoat)

[ScanMountGoat](https://github.com/ScanMountGoat)

[on Jan 1, 2023on Jan 1, 2023](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1368479975)

MemberAuthor

More actions

I've updated the ssbh\_data code to print out more of the anim 1.2 buffer types based on what you posted. You can run it as `cargo run -p ssbh_data_json front.nuanmb out.json` to print the information to the console. I've added three new header values that seem to use some sort of compression. The next step is to figure out how many items are in the compressed buffers. You can calculate the size per element since the buffer size is known. The floats at the beginning of the buffer may serve the same purpose as the min, max, and default values in version 2.0.

[![descatal](https://avatars.githubusercontent.com/u/54780311?u=d94664f81c929a793cc525730c2bbec90568e778&v=4&size=80)](https://github.com/descatal)

### descatal commented on Jan 2, 2023on Jan 2, 2023

[![@descatal](https://avatars.githubusercontent.com/u/54780311?u=d94664f81c929a793cc525730c2bbec90568e778&v=4&size=48)](https://github.com/descatal)

[descatal](https://github.com/descatal)

[on Jan 2, 2023on Jan 2, 2023](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1368841635)

More actions

Thanks, I can visualize it better with the printout.

For header 0x4308 (V12Test3), there's another 9 float after the keyframe (unk2) and before the start of the compression buffers.

Interestingly, I think 0x4409's unk5 is definitely float related to XYZW, since on UDE\_R originally does not have any rotation in XY, and the values show the same.

unk5:

0.0, 0.0, -0.416197, 0.909275,

0.0, 0.0, -0.314448, 0.949275,

0.0, 0.0, -0.186356, 0.982482

As for figuring out how many items are there in the compressed buffers, could you elaborate more? I cannot find a pattern between the buffer size and the number of keyframe.

I do notice that the buffer size will always align to 8 byte sizes, was this the case for v2.0+ anims as well?

[![ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=80)](https://github.com/ScanMountGoat)

### ScanMountGoat commented on Jan 2, 2023on Jan 2, 2023

[![@ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=48)](https://github.com/ScanMountGoat)

[ScanMountGoat](https://github.com/ScanMountGoat)

[on Jan 2, 2023on Jan 2, 2023](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1369293349) · edited by [ScanMountGoat](https://github.com/ScanMountGoat)

Edits

MemberAuthor

More actions

I do see a pattern with unk4 and the number of non constant float components. The bits per entry for version 2.0 is summed over the bits used to represent each field. If a value does not change for the track, it takes 0 bits. Quaternions are assumed to be normalized, so the fourth component can be inferred using only a single bit for the sign. It looks like version 1.2 could be using similar logic for calculating the bits per entry. I'm assuming each non constant component uses the same number of bits.

I'm not sure how to calculate the number of elements in the compressed buffer. Version 2.0+ uses offsets into a single buffer, and I don't believe there are any alignment requirements on the data size for each track for 2.0+.

[![descatal](https://avatars.githubusercontent.com/u/54780311?u=d94664f81c929a793cc525730c2bbec90568e778&v=4&size=80)](https://github.com/descatal)

### descatal commented on Jan 3, 2023on Jan 3, 2023

[![@descatal](https://avatars.githubusercontent.com/u/54780311?u=d94664f81c929a793cc525730c2bbec90568e778&v=4&size=48)](https://github.com/descatal)

[descatal](https://github.com/descatal)

[on Jan 3, 2023on Jan 3, 2023](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1369615012)

More actions

Thanks for the reply, as for the compression buffer parsing I think I'll do more investigation on it when I have access to my PS4 by next week, the decompiled code should give us some clue as how the compressed data is read and parsed.

That being said, I've already honed in on one subroutine that reads from the compressed buffer, and have decompiled the assembly code. I am not sure if the decompiled subroutine is the common decompression logic for every 1.2 header, but weirdly enough it reads 8 compressed bytes for each item.

I'll post the code [here](https://github.com/ultimate-research/ssbh_lib/files/10335797/nuanmb_parse.txt) just in case you recognize some of the logic that is similar with v2.0+ way of decompression.

As for the animation data that's stored in the nuamb that I've sent you, the decompressed values should correspond to the animation data that I've attached [here](https://github.com/ultimate-research/ssbh_lib/files/10335819/Front_Model.zip). (Same animation from older game).

I'll keep you posted if I find anything new, but I'll probably need to wait until next week before I can dive deeper into the decompiled code since I don't have access to my PS4 for now.

Really thank you for your time in helping out, this has cleared a lot of confusion for me and now I have a better direction to do more investigation.

[![ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=80)](https://github.com/ScanMountGoat)

### ScanMountGoat commented on Jan 5, 2023on Jan 5, 2023

[![@ScanMountGoat](https://avatars.githubusercontent.com/u/23301691?u=f7270ad8982acd99d132a827271b59eef666eaa1&v=4&size=48)](https://github.com/ScanMountGoat)

[ScanMountGoat](https://github.com/ScanMountGoat)

[on Jan 5, 2023on Jan 5, 2023](https://github.com/ultimate-research/ssbh_lib/issues/108#issuecomment-1372902900)

MemberAuthor

More actions

> As for the animation data that's stored in the nuamb that I've sent you, the decompressed values should correspond to the animation data that I've attached [here](https://github.com/ultimate-research/ssbh_lib/files/10335819/Front_Model.zip). (Same animation from older game).

Assuming these values are accurate, you should be able to approximate what the decompressed values should be for all the transform tracks. The collada file is using transformation matrices that can be decomposed to a translation vector, scale vector, and rotation quaternion. Here's the result using [glam](https://crates.io/crates/glam) in Rust. This seems to match up with the values stored in the header. The first value seems to be the first frame, the second value is somewhere in the middle, and the last value is the last frame. There's going to be a lot of rounding errors after converting to DAE and then decomposing the matrix again, so I wouldn't expect any of the values to match exactly.

```
use glam::Mat4;
// transforms is an array of matrix values from the collada DAE
for values in transforms {
    let transform = Mat4::from_cols_slice(&values).transpose();
    println!("{:?}", transform.to_scale_rotation_translation().1);
}
```

The result for the above code on the UDE\_R transformation from the DAE.

```
Quat(0.0, 0.0, -0.41620237, 0.909272)
Quat(0.0, 0.0, -0.49029186, 0.8715584)
Quat(-0.0, 0.0, -0.5975283, 0.8018479)
Quat(-0.0, 0.0, -0.64493644, 0.7642362)
Quat(-0.0, 0.0, -0.61650586, 0.7873503)
Quat(0.0, 0.0, -0.5587414, 0.829342)
Quat(0.0, 0.0, -0.506862, 0.86202717)
Quat(0.0, 0.0, -0.4304422, 0.90261805)
Quat(0.0, 0.0, -0.3319039, 0.94331324)
Quat(0.0, 0.0, -0.24423937, 0.96971494)
Quat(-0.0, 0.0, -0.24013875, 0.9707386)
Quat(-0.0, 0.0, -0.22340807, 0.97472507)
Quat(-0.0, 0.0, -0.22185467, 0.9750798)
Quat(-0.0, 0.0, -0.22845306, 0.9735549)
Quat(-0.0, 0.0, -0.21652246, 0.9762777)
Quat(0.0, 0.0, -0.2019185, 0.97940224)
Quat(-0.0, 0.0, -0.20421411, 0.9789263)
Quat(0.0, 0.0, -0.20667687, 0.9784093)
Quat(-0.0, 0.0, -0.21130945, 0.9774192)
Quat(0.0, 0.0, -0.22161676, 0.9751339)
Quat(0.0, -0.0, -0.23411381, 0.97220916)
Quat(0.0, 0.0, -0.2505164, 0.9681124)
Quat(-0.0, 0.0, -0.27091384, 0.9626036)
Quat(0.0, 0.0, -0.2901989, 0.9569664)
Quat(0.0, 0.0, -0.30229077, 0.9532157)
Quat(0.0, 0.0, -0.3019405, 0.9533268)
Quat(-0.0, 0.0, -0.30860215, 0.95119125)
Quat(0.0, 0.0, -0.31471795, 0.9491853)
Quat(0.0, -0.0, -0.31878373, 0.94782746)
Quat(-0.0, 0.0, -0.31932923, 0.94764394)
Quat(-0.0, 0.0, -0.31677523, 0.9485007)
Quat(0.0, 0.0, -0.31408837, 0.9493938)
Quat(0.0, 0.0, -0.31336758, 0.9496319)
Quat(-0.0, 0.0, -0.31445202, 0.9492733)
Quat(0.0, 0.0, -0.31413725, 0.94937766)
Quat(0.0, 0.0, -0.3048584, 0.95239764)
Quat(0.0, -0.0, -0.28346053, 0.95898396)
Quat(-0.0, 0.0, -0.25319645, 0.9674148)
Quat(0.0, 0.0, -0.21759303, 0.97603965)
Quat(-0.0, 0.0, -0.18636286, 0.982481)
```
The basic process for new versions like this is correctly rebuild binary file with ssbh_lib -> correctly rebuild ssbh_lib structs with ssbh_data -> adjust ssbh_data structs and test again.