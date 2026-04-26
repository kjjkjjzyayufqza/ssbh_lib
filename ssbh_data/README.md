# ssbh_data [![Latest Version](https://img.shields.io/crates/v/ssbh_data.svg)](https://crates.io/crates/ssbh_data) [![docs.rs](https://docs.rs/ssbh_data/badge.svg)](https://docs.rs/ssbh_data)  
A higher level data access layer for some SSBH formats. ssbh_data provides a more intuitive and minimal API where possible. SSBH types like `SsbhArray` and `SsbhString8` are replaced with their standard Rust equivalents of `Vec` and `String`. The decoding and encoding of binary buffers is handled automatically for formats like mesh and anim. Python bindings are available with [ssbh_data_py](https://github.com/ScanMountGoat/ssbh_data_py). 

## Supported Formats
| Format | Supported Versions (major.minor) | Read | Save |
| --- | --- | --- | --- |
| Modl (`.numdlb`, `.nusrcmdlb`) | 1.7 | :heavy_check_mark: | :heavy_check_mark: |
| Mesh (`.numshb`) | 1.8, 1.9, 1.10 | :heavy_check_mark: | :heavy_check_mark: |
| Skel (`.nusktb`) | 1.0 | :heavy_check_mark: | :heavy_check_mark: |
| Anim (`.nuanmb`) | 1.2, 2.0, 2.1 | :heavy_check_mark: | :heavy_check_mark: (2.0, 2.1); 1.2 save **experimental** |
| Matl (`.numatb`) | 1.5, 1.6 | :heavy_check_mark: | :heavy_check_mark: |
| Hlpb (`.nuhlpb`) | 1.1 | :heavy_check_mark: | :heavy_check_mark: |

**Anim v1.2 (experimental):** Reading and writing Anim v1.2 (`.nuanmb`) via `AnimData` is supported, including uncompressed and compressed (e.g. EXVS2-style) v1.2 writers. Treat save paths and binary round-trips as **experimental**—validate output against known-good files or in-game before relying on it.

ssbh_data also has support for Adj (`.adjb`) and MeshEx (`.numshexb`) files.