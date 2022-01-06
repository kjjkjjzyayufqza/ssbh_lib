//! # ssbh_lib
//!
//! ssbh_lib is a library for safe and efficient reading and writing of the SSBH binary formats used by Super Smash Bros Ultimate and some other games.
//!
//! ## Getting Started
//! ### Reading
//! If the file type isn't known, try all available SSBH types.
//!```no_run
//!# fn main() -> Result<(), Box<dyn std::error::Error>> {
//!let ssbh_data = ssbh_lib::Ssbh::from_file("unknown_data.bin")?;
//!match ssbh_data.data {
//!    ssbh_lib::SsbhFile::Hlpb(data) => println!("{:?}", data),
//!    ssbh_lib::SsbhFile::Matl(data) => println!("{:?}", data),
//!    ssbh_lib::SsbhFile::Modl(data) => println!("{:?}", data),
//!    ssbh_lib::SsbhFile::Mesh(data) => println!("{:?}", data),
//!    ssbh_lib::SsbhFile::Skel(data) => println!("{:?}", data),
//!    ssbh_lib::SsbhFile::Anim(data) => println!("{:?}", data),
//!    ssbh_lib::SsbhFile::Nrpd(data) => println!("{:?}", data),
//!    ssbh_lib::SsbhFile::Nufx(data) => println!("{:?}", data),
//!    ssbh_lib::SsbhFile::Shdr(data) => println!("{:?}", data),
//!}
//!# Ok(())
//!# }
//!```
//! In most cases, it's possible to infer the type of file based on its extension.
//!```no_run
//!# fn main() -> Result<(), Box<dyn std::error::Error>> {
//!let mesh = ssbh_lib::formats::mesh::Mesh::from_file("model.numshb")?;
//!# Ok(())
//!# }
//!```
//! See the documentation for [Ssbh] or any of the format types for additional reading methods.
//! ### Writing
//!```no_run
//!# fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mesh = ssbh_lib::formats::modl::Modl {
//! #     major_version: 1,
//! #     minor_version: 1,
//! #     model_name: "".into(),
//! #     skeleton_file_name: "".into(),
//! #     material_file_names: Vec::new().into(),
//! #     animation_file_name: ssbh_lib::RelPtr64::new("".into()),
//! #     mesh_file_name: "".into(),
//! #     entries: Vec::new().into(),
//! # };
//!let mut writer = std::io::Cursor::new(Vec::new());
//!mesh.write(&mut writer)?;
//!# Ok(())
//!# }
//!```
//! For the best performance when writing directly to a file, it's recommended to use the buffered `write_to_file` methods.
//!```no_run
//!# fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # let mesh = ssbh_lib::formats::modl::Modl {
//! #     major_version: 1,
//! #     minor_version: 1,
//! #     model_name: "".into(),
//! #     skeleton_file_name: "".into(),
//! #     material_file_names: Vec::new().into(),
//! #     animation_file_name: ssbh_lib::RelPtr64::new("".into()),
//! #     mesh_file_name: "".into(),
//! #     entries: Vec::new().into(),
//! # };
//!mesh.write_to_file("model.numshb")?;
//!# Ok(())
//!# }
//!```
//!
//! ## Derive Macros
//! The majority of the reading and writing code is automatically generated from the struct and type definitions using procedural macros.
//! [binread_derive](https://crates.io/crates/binread_derive) generates the parsing code and [ssbh_write_derive](https://crates.io/crates/ssbh_write_derive) generates the exporting code.
//! Any changes to structs, enums, or other types used to define a file format will be automatically reflected in the generated read and write functions when the code is rebuilt.
//!
//! This eliminates the need to write tedious and error prone code for parsing and exporting binary data.
//! The use of procedural macros and provided types such as [SsbhString] and [SsbhArray] enforce the conventions used
//! by the SSBH format for calculating relative offsets and alignment.
//!
//! ## Examples
//! A traditional struct definition for SSBH data may look like the following.
//! ```rust
//! struct FileData {
//!     name: u64,
//!     name_offset: u64,
//!     values_offset: u64,
//!     values_count: u64
//! }
//!```
//! The `FileData` struct has the correct size to represent the data on disk but has a number of issues.
//! The `values` array doesn't capture the fact that SSBH arrays are strongly typed.
//! It's not clear if the `name_offset` is an offset relative to the current position or some other buffer stored elsewhere in the file.
//!
//! Composing a combination of predefined SSBH types such as [SsbhString] with additional types implementing [SsbhWrite] and [BinRead]
//! improves the amount of type information for the data and makes the usage of offsets less ambiguous.
//! ```rust
//!
//! use ssbh_lib::{SsbhArray, RelPtr64, SsbhString};
//! use ssbh_write::SsbhWrite;
//! use binread::BinRead;
//!
//! #[derive(BinRead, SsbhWrite)]
//! struct FileData {
//!     name: SsbhString,
//!     name_offset: RelPtr64<SsbhString>,
//!     values: SsbhArray<u32>    
//! }
//! # fn main() {}
//! ```
//! Now it's clear that `name` and `name_offset` are both null terminated strings, but `name_offset` has one more level of indirection.
//! In addition, `values` now has the correct typing information. The element count can be correctly calculated as `values.elements.len()`.
//! The reading and writing code is generated automatically by adding `#[derive(BinRead, SsbhWrite)]` to the struct.
pub mod formats;

mod arrays;
pub use arrays::{SsbhArray, SsbhByteBuffer};

mod vectors;
pub use vectors::{Color4f, Matrix3x3, Matrix4x4, Vector3, Vector4};

mod strings;
pub use strings::{CString, InlineString, SsbhString, SsbhString8};

mod enums;
pub use enums::SsbhEnum64;

// TODO: This should just be part of a prelude?
pub use formats::adj::Adj;
pub use formats::anim::Anim;
pub use formats::hlpb::Hlpb;
pub use formats::matl::Matl;
pub use formats::mesh::Mesh;
pub use formats::meshex::MeshEx;
pub use formats::modl::Modl;
pub use formats::nrpd::Nrpd;
pub use formats::nufx::Nufx;
pub use formats::shdr::Shdr;
pub use formats::skel::Skel;

use self::formats::*;
use binread::io::Cursor;
use binread::BinReaderExt;
use binread::{
    io::{Read, Seek, SeekFrom},
    BinRead, BinResult, ReadOptions,
};

use ssbh_write::SsbhWrite;
use std::convert::TryFrom;
use std::fs;
use std::io::Write;
use std::marker::PhantomData;
use std::path::Path;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

impl Ssbh {
    /// Tries to read one of the SSBH types from `path`.
    /// The entire file is buffered for performance.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = Cursor::new(fs::read(path)?);
        let ssbh = file.read_le::<Ssbh>()?;
        Ok(ssbh)
    }

    /// Tries to read one of the SSBH types from `reader`.
    /// For best performance when opening from a file, use `from_file` instead.
    pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, Box<dyn std::error::Error>> {
        let ssbh = reader.read_le::<Ssbh>()?;

        Ok(ssbh)
    }

    /// Writes the data to the given writer.
    /// For best performance when writing to a file, use `write_to_file` instead.
    pub fn write<W: std::io::Write + Seek>(&self, writer: &mut W) -> std::io::Result<()> {
        write_ssbh_header_and_data(writer, &self.data)?;
        Ok(())
    }

    /// Writes the data to the given path.
    /// The entire file is buffered for performance.
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        write_buffered(&mut file, |c| write_ssbh_header_and_data(c, &self.data))?;
        Ok(())
    }
}

/// Errors while reading SSBH files.
pub enum ReadSsbhError {
    /// An error occurred while trying to read the file.
    BinRead(binread::error::Error),
    /// An error occurred while trying to read the file.
    Io(std::io::Error),
    /// The type of SSBH file did not match the expected SSBH type.
    InvalidSsbhType,
}

impl std::error::Error for ReadSsbhError {}

impl From<binread::error::Error> for ReadSsbhError {
    fn from(e: binread::error::Error) -> Self {
        Self::BinRead(e)
    }
}

impl From<std::io::Error> for ReadSsbhError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl std::fmt::Display for ReadSsbhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl std::fmt::Debug for ReadSsbhError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadSsbhError::InvalidSsbhType => {
                write!(
                    f,
                    "The type of SSBH file did not match the expected SSBH type."
                )
            }
            ReadSsbhError::BinRead(err) => write!(f, "BinRead Error: {:?}", err),
            ReadSsbhError::Io(err) => write!(f, "IO Error: {:?}", err),
        }
    }
}

macro_rules! ssbh_read_write_impl {
    ($ty:ident, $ty2:path, $magic:expr) => {
        impl $ty {
            /// Tries to read the current SSBH type from `path`.
            /// The entire file is buffered for performance.
            pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ReadSsbhError> {
                let mut file = Cursor::new(fs::read(path)?);
                let ssbh = file.read_le::<Ssbh>()?;
                match ssbh.data {
                    $ty2(v) => Ok(v),
                    _ => Err(ReadSsbhError::InvalidSsbhType),
                }
            }

            /// Tries to read the current SSBH type from `reader`.
            /// For best performance when opening from a file, use `from_file` instead.
            pub fn read<R: Read + Seek>(reader: &mut R) -> Result<Self, ReadSsbhError> {
                let ssbh = reader.read_le::<Ssbh>()?;
                match ssbh.data {
                    $ty2(v) => Ok(v),
                    _ => Err(ReadSsbhError::InvalidSsbhType),
                }
            }

            /// Tries to write the SSBH type to `writer`.
            /// For best performance when writing to a file, use `write_to_file` instead.
            pub fn write<W: std::io::Write + Seek>(&self, writer: &mut W) -> std::io::Result<()> {
                write_ssbh_file(writer, self, $magic)?;
                Ok(())
            }

            /// Tries to write the current SSBH type to `path`.
            /// The entire file is buffered for performance.
            pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
                let mut file = std::fs::File::create(path)?;
                write_buffered(&mut file, |c| write_ssbh_file(c, self, $magic))?;
                Ok(())
            }
        }
    };
}

macro_rules! read_write_impl {
    ($ty:ident) => {
        impl $ty {
            /// Tries to read the type from `path`.
            /// The entire file is buffered for performance.
            pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
                let mut file = Cursor::new(fs::read(path)?);
                let value = file.read_le::<$ty>()?;
                Ok(value)
            }

            /// Tries to read the type from `reader`.
            /// For best performance when opening from a file, use `from_file` instead.
            pub fn read<R: Read + Seek>(
                reader: &mut R,
            ) -> Result<Self, Box<dyn std::error::Error>> {
                let value = reader.read_le::<$ty>()?;
                Ok(value)
            }

            /// Tries to write the type to `writer`.
            /// For best performance when writing to a file, use `write_to_file` instead.
            pub fn write<W: std::io::Write + Seek>(&self, writer: &mut W) -> std::io::Result<()> {
                SsbhWrite::write(self, writer)?;
                Ok(())
            }

            /// Tries to write the type to `path`.
            /// The entire file is buffered for performance.
            pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
                let mut file = std::fs::File::create(path)?;
                write_buffered(&mut file, |c| self.write(c))?;
                Ok(())
            }
        }
    };
}

ssbh_read_write_impl!(Hlpb, SsbhFile::Hlpb, b"BPLH");
ssbh_read_write_impl!(Matl, SsbhFile::Matl, b"LTAM");
ssbh_read_write_impl!(Modl, SsbhFile::Modl, b"LDOM");
ssbh_read_write_impl!(Mesh, SsbhFile::Mesh, b"HSEM");
ssbh_read_write_impl!(Skel, SsbhFile::Skel, b"LEKS");
ssbh_read_write_impl!(Anim, SsbhFile::Anim, b"MINA");
ssbh_read_write_impl!(Nrpd, SsbhFile::Nrpd, b"DPRN");
ssbh_read_write_impl!(Nufx, SsbhFile::Nufx, b"XFUN");
ssbh_read_write_impl!(Shdr, SsbhFile::Shdr, b"RDHS");

read_write_impl!(MeshEx);
read_write_impl!(Adj);

pub(crate) fn absolute_offset_checked(
    position: u64,
    relative_offset: u64,
) -> Result<u64, binread::Error> {
    // Overflow can occur when the offset is actually a signed integer like -1i64 (0xFFFFFFFF FFFFFFFF).
    // Use checked addition to convert the panic to a result to avoid terminating the program.
    match position.checked_add(relative_offset) {
        Some(offset) => Ok(offset),
        // TODO: Use a different error variant?
        None => Err(binread::error::Error::AssertFail {
            pos: position,
            message: format!(
                "Overflow occurred while computing relative offset {}",
                relative_offset
            ),
        }),
    }
}

// TODO: This should probably be sealed?
pub trait Offset:
    Into<u64> + TryFrom<u64> + SsbhWrite + BinRead<Args = ()> + Default + PartialEq
{
}
impl Offset for u8 {}
impl Offset for u16 {}
impl Offset for u32 {}
impl Offset for u64 {}

/// A file pointer relative to the start of the reader.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug)]
#[repr(transparent)]
pub struct Ptr<P: Offset, T: BinRead<Args = ()>>(
    Option<T>,
    #[cfg_attr(feature = "serde", serde(skip))] PhantomData<P>,
);

// TODO: Find a way to reuse these bounds?
// TODO: Create an Offset trait and implement it for the unsigned types no bigger than u64?
impl<P: Offset, T: BinRead<Args = ()>> Ptr<P, T> {
    /// Creates an absolute offset for a value that is not null.
    pub fn new(value: T) -> Self {
        Self(Some(value), PhantomData::<P>)
    }

    /// Creates an absolute offset for a null value.
    pub fn null() -> Self {
        Self(None, PhantomData::<P>)
    }
}

/// A 16 bit file pointer relative to the start of the reader.
pub type Ptr16<T> = Ptr<u16, T>;

/// A 32 bit file pointer relative to the start of the reader.
pub type Ptr32<T> = Ptr<u32, T>;

/// A 64 bit file pointer relative to the start of the reader.
pub type Ptr64<T> = Ptr<u64, T>;

impl<P: Offset, T: BinRead<Args = ()>> BinRead for Ptr<P, T> {
    type Args = T::Args;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        options: &ReadOptions,
        args: Self::Args,
    ) -> BinResult<Self> {
        let offset = P::read_options(reader, options, ())?;
        if offset == P::default() {
            return Ok(Self::null());
        }

        let saved_pos = reader.stream_position()?;

        reader.seek(SeekFrom::Start(offset.into()))?;
        let value = T::read_options(reader, options, args)?;

        reader.seek(SeekFrom::Start(saved_pos))?;

        Ok(Self::new(value))
    }
}

impl<P: Offset, T: BinRead<Args = ()>> core::ops::Deref for Ptr<P, T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A 64 bit file pointer relative to the start of the pointer type.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug)]
#[repr(transparent)]
pub struct RelPtr64<T: BinRead>(Option<T>);

impl<T: BinRead> RelPtr64<T> {
    /// Creates a relative offset for `value` that is not null.
    pub fn new(value: T) -> Self {
        Self(Some(value))
    }

    /// Creates a relative offset for a null value.
    pub fn null() -> Self {
        Self(None)
    }
}

impl<T: BinRead + PartialEq> PartialEq for RelPtr64<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: BinRead + Eq> Eq for RelPtr64<T> {}

impl<T: BinRead> From<Option<T>> for RelPtr64<T> {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(v) => Self::new(v),
            None => Self::null(),
        }
    }
}

impl<T: BinRead> BinRead for RelPtr64<T> {
    type Args = T::Args;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        options: &ReadOptions,
        args: Self::Args,
    ) -> BinResult<Self> {
        let pos_before_read = reader.stream_position()?;

        let relative_offset = u64::read_options(reader, options, ())?;
        if relative_offset == 0 {
            return Ok(Self::null());
        }

        let saved_pos = reader.stream_position()?;

        let seek_pos = absolute_offset_checked(pos_before_read, relative_offset)?;
        reader.seek(SeekFrom::Start(seek_pos))?;
        let value = T::read_options(reader, options, args)?;

        reader.seek(SeekFrom::Start(saved_pos))?;

        Ok(Self(Some(value)))
    }
}

impl<T: BinRead> core::ops::Deref for RelPtr64<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The container type for the various SSBH formats.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug)]
#[br(magic = b"HBSS")]
pub struct Ssbh {
    #[br(align_before = 0x10)]
    pub data: SsbhFile,
}

/// The associated magic and format for each SSBH type.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(BinRead, Debug)]
pub enum SsbhFile {
    #[br(magic = b"BPLH")]
    Hlpb(hlpb::Hlpb),

    #[br(magic = b"LTAM")]
    Matl(matl::Matl),

    #[br(magic = b"LDOM")]
    Modl(modl::Modl),

    #[br(magic = b"HSEM")]
    Mesh(mesh::Mesh),

    #[br(magic = b"LEKS")]
    Skel(skel::Skel),

    #[br(magic = b"MINA")]
    Anim(anim::Anim),

    #[br(magic = b"DPRN")]
    Nrpd(nrpd::Nrpd),

    #[br(magic = b"XFUN")]
    Nufx(nufx::Nufx),

    #[br(magic = b"RDHS")]
    Shdr(shdr::Shdr),
}

/// A wrapper type that serializes the value and absolute offset of the start of the value
/// to aid in debugging.
#[cfg(feature = "serde")]
#[derive(Debug, Serialize, Deserialize, SsbhWrite)]
pub struct DebugPosition<T: BinRead<Args = ()> + Serialize + SsbhWrite> {
    val: T,
    pos: u64,
}

#[cfg(feature = "serde")]
impl<T> BinRead for DebugPosition<T>
where
    T: BinRead<Args = ()> + Serialize + SsbhWrite,
{
    type Args = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        options: &ReadOptions,
        _args: Self::Args,
    ) -> BinResult<Self> {
        let pos = reader.stream_position()?;
        let val = T::read_options(reader, options, ())?;
        Ok(Self { val, pos })
    }
}

pub(crate) fn round_up(value: u64, n: u64) -> u64 {
    // Find the next largest multiple of n.
    ((value + n - 1) / n) * n
}

pub(crate) fn write_relative_offset<W: Write + Seek>(
    writer: &mut W,
    data_ptr: &u64,
) -> std::io::Result<()> {
    let current_pos = writer.stream_position()?;
    u64::write(&(*data_ptr - current_pos), writer)?;
    Ok(())
}

fn write_rel_ptr_aligned_specialized<
    W: Write + Seek,
    T,
    F: Fn(&T, &mut W, &mut u64) -> std::io::Result<()>,
>(
    writer: &mut W,
    data: &Option<T>,
    data_ptr: &mut u64,
    alignment: u64,
    write_t: F,
) -> std::io::Result<()> {
    match data {
        Some(value) => {
            // Calculate the relative offset.
            *data_ptr = round_up(*data_ptr, alignment);
            write_relative_offset(writer, data_ptr)?;

            // Write the data at the specified offset.
            let pos_after_offset = writer.stream_position()?;
            writer.seek(SeekFrom::Start(*data_ptr))?;

            // Allow custom write functions for performance reasons.
            write_t(value, writer, data_ptr)?;

            // Point the data pointer past the current write.
            // Types with relative offsets will already increment the data pointer.
            let current_pos = writer.stream_position()?;
            if current_pos > *data_ptr {
                *data_ptr = round_up(current_pos, alignment);
            }

            writer.seek(SeekFrom::Start(pos_after_offset))?;
            Ok(())
        }
        None => {
            // Null offsets don't increment the data pointer.
            u64::write(&0u64, writer)?;
            Ok(())
        }
    }
}

fn write_rel_ptr_aligned<W: Write + Seek, T: SsbhWrite>(
    writer: &mut W,
    data: &Option<T>,
    data_ptr: &mut u64,
    alignment: u64,
) -> std::io::Result<()> {
    write_rel_ptr_aligned_specialized(writer, data, data_ptr, alignment, T::ssbh_write)?;
    Ok(())
}

fn write_ssbh_header<W: Write + Seek>(writer: &mut W, magic: &[u8; 4]) -> std::io::Result<()> {
    // Hardcode the header because this is shared for all SSBH formats.
    writer.write_all(b"HBSS")?;
    u64::write(&64u64, writer)?;
    u32::write(&0u32, writer)?;
    writer.write_all(magic)?;
    Ok(())
}

impl<P: Offset, T: SsbhWrite + BinRead<Args = ()>> SsbhWrite for Ptr<P, T> {
    fn ssbh_write<W: Write + Seek>(
        &self,
        writer: &mut W,
        data_ptr: &mut u64,
    ) -> std::io::Result<()> {
        // TODO: This is nearly identical to the relative pointer function.
        // The data pointer must point past the containing struct.
        let current_pos = writer.stream_position()?;
        if *data_ptr < current_pos + self.size_in_bytes() {
            *data_ptr = current_pos + self.size_in_bytes();
        }

        match &self.0 {
            Some(value) => {
                let alignment = T::alignment_in_bytes();

                // The data pointer must point past the containing type.
                let current_pos = writer.stream_position()?;
                if *data_ptr < current_pos + self.size_in_bytes() {
                    *data_ptr = current_pos + self.size_in_bytes();
                }

                // Calculate the absolute offset.
                *data_ptr = round_up(*data_ptr, alignment);

                let offset = P::try_from(*data_ptr).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "Failed to convert offset {} to a pointer with {} bytes.",
                            data_ptr,
                            std::mem::size_of::<P>()
                        ),
                    )
                })?;
                P::ssbh_write(&offset, writer, data_ptr)?;

                // Write the data at the specified offset.
                let pos_after_offset = writer.stream_position()?;
                writer.seek(SeekFrom::Start(*data_ptr))?;

                value.ssbh_write(writer, data_ptr)?;

                // Point the data pointer past the current write.
                // Types with relative offsets will already increment the data pointer.
                let current_pos = writer.stream_position()?;
                if current_pos > *data_ptr {
                    *data_ptr = round_up(current_pos, alignment);
                }

                writer.seek(SeekFrom::Start(pos_after_offset))?;
                Ok(())
            }
            None => {
                P::default().ssbh_write(writer, data_ptr)?;
                Ok(())
            }
        }
    }

    fn size_in_bytes(&self) -> u64 {
        // TODO: Use the size_in_bytes already defined for P?
        std::mem::size_of::<P>() as u64
    }
}

impl<T: SsbhWrite + binread::BinRead> SsbhWrite for RelPtr64<T> {
    fn ssbh_write<W: Write + Seek>(
        &self,
        writer: &mut W,
        data_ptr: &mut u64,
    ) -> std::io::Result<()> {
        // The data pointer must point past the containing struct.
        let current_pos = writer.stream_position()?;
        if *data_ptr < current_pos + self.size_in_bytes() {
            *data_ptr = current_pos + self.size_in_bytes();
        }

        write_rel_ptr_aligned(writer, &self.0, data_ptr, T::alignment_in_bytes())?;

        Ok(())
    }

    fn size_in_bytes(&self) -> u64 {
        8
    }
}

pub(crate) fn write_ssbh_header_and_data<W: Write + Seek>(
    writer: &mut W,
    data: &SsbhFile,
) -> std::io::Result<()> {
    match &data {
        SsbhFile::Modl(modl) => write_ssbh_file(writer, modl, b"LDOM"),
        SsbhFile::Skel(skel) => write_ssbh_file(writer, skel, b"LEKS"),
        SsbhFile::Nufx(nufx) => write_ssbh_file(writer, nufx, b"XFUN"),
        SsbhFile::Shdr(shdr) => write_ssbh_file(writer, shdr, b"RDHS"),
        SsbhFile::Matl(matl) => write_ssbh_file(writer, matl, b"LTAM"),
        SsbhFile::Anim(anim) => write_ssbh_file(writer, anim, b"MINA"),
        SsbhFile::Hlpb(hlpb) => write_ssbh_file(writer, hlpb, b"BPLH"),
        SsbhFile::Mesh(mesh) => write_ssbh_file(writer, mesh, b"HSEM"),
        SsbhFile::Nrpd(nrpd) => write_ssbh_file(writer, nrpd, b"DPRN"),
    }
}

pub(crate) fn write_buffered<
    W: Write + Seek,
    F: Fn(&mut Cursor<Vec<u8>>) -> std::io::Result<()>,
>(
    writer: &mut W,
    write_data: F,
) -> std::io::Result<()> {
    // Buffer the entire write operation into memory to improve performance.
    // The seeks used to write relative offsets cause flushes for BufWriter.
    let mut cursor = Cursor::new(Vec::new());
    write_data(&mut cursor)?;

    writer.write_all(cursor.get_mut())?;
    Ok(())
}

// TODO: This can probably just be derived.
pub(crate) fn write_ssbh_file<W: Write + Seek, S: SsbhWrite>(
    writer: &mut W,
    data: &S,
    magic: &[u8; 4],
) -> std::io::Result<()> {
    write_ssbh_header(writer, magic)?;
    let mut data_ptr = writer.stream_position()?;

    // Point past the struct.
    data_ptr += data.size_in_bytes(); // size of fields

    data.ssbh_write(writer, &mut data_ptr)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexlit::hex;

    #[test]
    fn new_relptr64() {
        let ptr = RelPtr64::new(5u32);
        assert_eq!(Some(5u32), ptr.0);
    }

    #[test]
    fn read_relptr() {
        let mut reader = Cursor::new(hex!("09000000 00000000 05070000"));
        let value = reader.read_le::<RelPtr64<u8>>().unwrap();
        assert_eq!(7u8, value.unwrap());

        // Make sure the reader position is restored.
        let value = reader.read_le::<u8>().unwrap();
        assert_eq!(5u8, value);
    }

    #[test]
    fn read_relptr_null() {
        let mut reader = Cursor::new(hex!("00000000 00000000 05070000"));
        let value = reader.read_le::<RelPtr64<u8>>().unwrap();
        assert_eq!(None, value.0);

        // Make sure the reader position is restored.
        let value = reader.read_le::<u8>().unwrap();
        assert_eq!(5u8, value);
    }

    #[test]
    fn read_relptr_offset_overflow() {
        let mut reader = Cursor::new(hex!("00000000 FFFFFFFF FFFFFFFF 05070000"));
        reader.seek(SeekFrom::Start(4)).unwrap();

        // Make sure this just returns an error instead.
        let result = reader.read_le::<RelPtr64<u8>>();
        assert!(matches!(
            result,
            Err(binread::error::Error::AssertFail { pos: 4, message })
            if message == format!(
                "Overflow occurred while computing relative offset {}",
                0xFFFFFFFFFFFFFFFFu64
            )
        ));

        // Make sure the reader position is restored.
        let value = reader.read_le::<u8>().unwrap();
        assert_eq!(5u8, value);
    }

    #[test]
    fn read_ptr8() {
        let mut reader = Cursor::new(hex!("04050000 07"));
        let value = reader.read_le::<Ptr<u8, u8>>().unwrap();
        assert_eq!(7u8, value.unwrap());

        // Make sure the reader position is restored.
        let value = reader.read_le::<u8>().unwrap();
        assert_eq!(5u8, value);
    }

    #[test]
    fn read_ptr64() {
        let mut reader = Cursor::new(hex!("09000000 00000000 05070000"));
        let value = reader.read_le::<Ptr64<u8>>().unwrap();
        assert_eq!(7u8, value.unwrap());

        // Make sure the reader position is restored.
        let value = reader.read_le::<u8>().unwrap();
        assert_eq!(5u8, value);
    }

    #[test]
    fn read_ptr_null() {
        let mut reader = Cursor::new(hex!("00000000 00000000 05070000"));
        let value = reader.read_le::<Ptr64<u8>>().unwrap();
        assert_eq!(None, value.0);

        // Make sure the reader position is restored.
        let value = reader.read_le::<u8>().unwrap();
        assert_eq!(5u8, value);
    }

    #[test]
    fn write_ptr16() {
        let value = Ptr16::<u8>::new(5u8);

        let mut writer = Cursor::new(Vec::new());
        let mut data_ptr = 0;
        value.ssbh_write(&mut writer, &mut data_ptr).unwrap();

        assert_eq!(writer.into_inner(), hex!("0200 05"));
        assert_eq!(3, data_ptr);
    }

    #[test]
    fn write_ptr32() {
        let value = Ptr32::<u8>::new(5u8);

        let mut writer = Cursor::new(Vec::new());
        let mut data_ptr = 0;
        value.ssbh_write(&mut writer, &mut data_ptr).unwrap();

        assert_eq!(writer.into_inner(), hex!("04000000 05"));
        assert_eq!(5, data_ptr);
    }

    #[test]
    fn write_null_ptr32() {
        let value = Ptr32::<u8>::null();

        let mut writer = Cursor::new(Vec::new());
        let mut data_ptr = 0;
        value.ssbh_write(&mut writer, &mut data_ptr).unwrap();

        assert_eq!(writer.into_inner(), hex!("00000000"));
        assert_eq!(4, data_ptr);
    }

    #[test]
    fn write_ptr64() {
        let value = Ptr64::<u8>::new(5u8);

        let mut writer = Cursor::new(Vec::new());
        let mut data_ptr = 0;
        value.ssbh_write(&mut writer, &mut data_ptr).unwrap();

        assert_eq!(writer.into_inner(), hex!("08000000 00000000 05"));
        assert_eq!(9, data_ptr);
    }

    #[test]
    fn write_ptr64_vec_u8() {
        // Check that the alignment uses the inner type's alignment.
        let value = Ptr64::new(vec![5u8]);

        let mut writer = Cursor::new(Vec::new());
        let mut data_ptr = 0;
        value.ssbh_write(&mut writer, &mut data_ptr).unwrap();

        assert_eq!(writer.into_inner(), hex!("08000000 00000000 05"));
        assert_eq!(9, data_ptr);
    }

    #[test]
    fn write_ptr64_vec_u32() {
        // Check that the alignment uses the inner type's alignment.
        let value = Ptr64::new(vec![5u32]);

        let mut writer = Cursor::new(Vec::new());
        let mut data_ptr = 0;
        value.ssbh_write(&mut writer, &mut data_ptr).unwrap();

        assert_eq!(writer.into_inner(), hex!("08000000 00000000 05000000"));
        assert_eq!(12, data_ptr);
    }

    #[test]
    fn write_null_rel_ptr() {
        let value = RelPtr64::<u32>(None);

        let mut writer = Cursor::new(Vec::new());
        let mut data_ptr = 0;
        value.ssbh_write(&mut writer, &mut data_ptr).unwrap();

        assert_eq!(writer.into_inner(), hex!("00000000 00000000"));
        assert_eq!(8, data_ptr);
    }

    #[test]
    fn write_nested_rel_ptr_depth2() {
        let value = RelPtr64::new(RelPtr64::new(7u32));

        let mut writer = Cursor::new(Vec::new());
        let mut data_ptr = 0;
        value.ssbh_write(&mut writer, &mut data_ptr).unwrap();

        assert_eq!(
            writer.into_inner(),
            hex!(
                "08000000 00000000 
                 08000000 00000000 
                 07000000"
            )
        );
        assert_eq!(20, data_ptr);
    }
}
