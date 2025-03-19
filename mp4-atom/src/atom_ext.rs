use crate::*;

// Combine the version and flags into a single struct
// We use a special trait to ensure it's always a u32
pub(crate) trait Ext: Default {
    fn encode(&self) -> Result<u32>;
    fn decode(v: u32) -> Result<Self>;
}

// Rather than encoding/decoding the header in every atom, use this trait.
pub(crate) trait AtomExt: Sized {
    const KIND_EXT: FourCC;

    // One day default associated types will be a thing, then this can be ()
    type Ext: Ext;

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<Self::Ext>;
    fn decode_body_ext<B: Buf>(buf: &mut B, ext: Self::Ext) -> Result<Self>;
}

impl<T: AtomExt> Atom for T {
    const KIND: FourCC = Self::KIND_EXT;

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        let ext = Ext::decode(u32::decode(buf)?)?;
        AtomExt::decode_body_ext(buf, ext)
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        // Here's the magic, we reserve space for the version/flags first
        let start = buf.len();
        0u32.encode(buf)?;

        // That way we can return them as part of the trait, avoiding boilerplate
        let ext = self.encode_body_ext(buf)?;

        // Go back and update the version/flags
        let header = ext.encode()?;
        buf.set_slice(start, &header.to_be_bytes());

        Ok(())
    }
}

// Some atoms don't have any version/flags, so we provide a default implementation
impl Ext for () {
    fn encode(&self) -> Result<u32> {
        Ok(0)
    }

    fn decode(_: u32) -> Result<()> {
        Ok(())
    }
}

// Here's a macro to make life easier:
/* input:
ext! {
    name: Tfdt,
    versions: [0, 1],
    flags: {
        base_data_offset = 0,
        sample_description_index = 1,
        default_sample_duration = 3,
        default_sample_size = 4,
        default_sample_flags = 5,
        duration_is_empty = 16,
        default_base_is_moof = 17,
    },
}

output:
enum TfdtVersion {
    V0 = 0,
    V1 = 1,
}

struct TfdtExt {
    pub version: TfdtVersion,
    pub base_data_offset: bool,
    pub sample_description_index: bool,
    pub default_sample_duration: bool,
    pub default_sample_size: bool,
    pub default_sample_flags: bool,
    pub duration_is_empty: bool,
    pub default_base_is_moof: bool,
}
*/

macro_rules! ext {
    (name: $name:ident, versions: [$($version:expr),*], flags: { $($flag:ident = $bit:expr,)* }) => {
        paste::paste! {
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub(crate) enum [<$name Version>] {
                $(
                    [<V $version>] = $version,
                )*
            }

            impl TryFrom<u8> for [<$name Version>] {
                type Error = Error;

                fn try_from(v: u8) -> Result<Self> {
                    match v {
                        $(
                            $version => Ok(Self::[<V $version>]),
                        )*
                        _ => Err(Error::UnknownVersion(v)),
                    }
                }
            }

            impl Default for [<$name Version>] {
                // Hilarious way to return the first version in the list
                #[allow(unreachable_code)]
                fn default() -> Self {
                    $(
                        return Self::[<V $version>];
                    )*
                }
            }

            #[derive(Debug, Clone, PartialEq, Eq, Default)]
            pub(crate) struct [<$name Ext>] {
                pub version: [<$name Version>],
                $(
                    pub $flag: bool,
                )*
            }

            impl Ext for [<$name Ext>] {
                fn encode(&self) -> Result<u32>{
                    Ok((self.version as u32) << 24 $(| (self.$flag as u32) << $bit)*)
                }

                fn decode(v: u32) -> Result<Self> {
                    Ok([<$name Ext>] {
                        version: [<$name Version>]::try_from((v >> 24) as u8)?,
                        $(
                            $flag: (v & (1 << $bit)) != 0,
                        )*
                    })
                }
            }

            // Helper when there are no flags
            impl From<[<$name Version>]> for [<$name Ext>] {
                fn from(version: [<$name Version>]) -> Self {
                    // Not using ..Default::default() to avoid Clippy
                    let mut ext = Self::default();
                    ext.version = version;
                    ext
                }
            }
        }
    };
}

pub(crate) use ext;
