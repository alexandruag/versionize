//! Here be module documentation.

use std::io::{self, Read, Write};
use std::mem::size_of;
use std::result;
use std::slice;

use vmm_sys_util::fam::{FamStruct, FamStructWrapper};

use crate::{VersionMap, Versionize, VersionizeResult};

/// An abstraction that provides a trivial `Versionize` implementation around the inner type,
/// which is interpreted as a "versionless" blob of bytes (i.e. its size never changes and no
/// special semantics are associated with the content).
// We can drop the `Default` requirement and rely on something like `MaybeUninit`, but this is
// cleaner as long as it's a valid approach.
#[derive(Default, Copy, Clone)]
#[repr(C)]
pub struct Blob<T>(pub T);

impl<T> Blob<T> {
    fn write_bytes<W: Write>(w: &mut W, t: &T) -> result::Result<(), io::Error> {
        let slice = unsafe { slice::from_raw_parts(t as *const T as *const u8, size_of::<T>()) };
        w.write_all(slice)
    }

    fn read_bytes<R: Read>(r: &mut R, t: &mut T) -> result::Result<(), io::Error> {
        let slice = unsafe { slice::from_raw_parts_mut(t as *mut T as *mut u8, size_of::<T>()) };
        r.read_exact(slice)
    }
}

impl<T: Default> Versionize for Blob<T> {
    fn serialize<W: Write>(
        &self,
        writer: &mut W,
        _version_map: &VersionMap,
        _target_version: u16,
    ) -> VersionizeResult<()> {
        // TODO: return a proper `VersionizeError` and/or use another way to write the bytes
        // if necessary. Same for the following `unwrap`s.
        Ok(Blob::<T>::write_bytes(writer, &self.0).unwrap())
    }

    fn deserialize<R: Read>(
        reader: &mut R,
        _version_map: &VersionMap,
        _source_version: u16,
    ) -> VersionizeResult<Self>
    where
        Self: Sized,
    {
        let mut obj = Blob::<T>::default();
        Blob::<T>::read_bytes(reader, &mut obj.0).unwrap();
        Ok(obj)
    }

    fn version() -> u16 {
        1
    }
}

/// This is the equivalent of `Blob` for a `FamStructWrapper` and its entries.
pub struct FamBlob<T: Default + FamStruct>(pub FamStructWrapper<T>);

impl<T: Default + FamStruct> Clone for FamBlob<T> {
    fn clone(&self) -> Self {
        FamBlob(self.0.clone())
    }
}

impl<T> Versionize for FamBlob<T>
where
    T: Default + FamStruct,
    T::Entry: Default,
{
    fn serialize<W: Write>(
        &self,
        mut writer: &mut W,
        version_map: &VersionMap,
        target_version: u16,
    ) -> VersionizeResult<()> {
        Blob::<T>::write_bytes(&mut writer, self.0.as_fam_struct_ref()).unwrap();
        // The sequence of calls up to `serialize` are essentially equivalent to the
        // `as_slice().to_vec()` from the `serialize` impl for `FamStructWrapper`.
        self.0
            .as_slice()
            .iter()
            .map(|x| Blob(*x))
            .collect::<Vec<_>>()
            .serialize(&mut writer, version_map, target_version)
    }

    fn deserialize<R: Read>(
        reader: &mut R,
        version_map: &VersionMap,
        source_version: u16,
    ) -> VersionizeResult<Self>
    where
        Self: Sized,
    {
        let header = Blob::<T>::deserialize(reader, version_map, source_version)?.0;
        let entries =
            Vec::<Blob<<T as FamStruct>::Entry>>::deserialize(reader, version_map, source_version)?
                .into_iter()
                // Take the inner value.
                .map(|b| b.0)
                .collect::<Vec<_>>();

        let mut obj = FamStructWrapper::from_entries(&entries);
        *obj.as_mut_fam_struct() = header;

        Ok(FamBlob(obj))
    }

    fn version() -> u16 {
        1
    }
}
