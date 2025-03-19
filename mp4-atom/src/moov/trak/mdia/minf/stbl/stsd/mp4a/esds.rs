use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Esds {
    pub es_desc: EsDescriptor,
}

impl AtomExt for Esds {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"esds");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let mut es_desc = None;

        while let Some(desc) = Descriptor::decode_maybe(buf)? {
            match desc {
                Descriptor::EsDescriptor(desc) => es_desc = Some(desc),
                Descriptor::Unknown(tag, _) => {
                    tracing::warn!("unknown descriptor: {:02X}", tag)
                }
                _ => return Err(Error::UnexpectedDescriptor(desc.tag())),
            }
        }

        Ok(Esds {
            es_desc: es_desc.ok_or(Error::MissingDescriptor(EsDescriptor::TAG))?,
        })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        Descriptor::from(self.es_desc).encode(buf)
    }
}

macro_rules! descriptors {
    ($($name:ident,)*) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum Descriptor {
            $(
                $name($name),
            )*
            Unknown(u8, Vec<u8>),
        }

        impl Decode for Descriptor {
            fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
                let tag = u8::decode(buf)?;

                let mut size: u32 = 0;
                for _ in 0..4 {
                    let b = u8::decode(buf)?;
                    size = (size << 7) | (b & 0x7F) as u32;
                    if b & 0x80 == 0 {
                        break;
                    }
                }

                match tag {
                    $(
                        $name::TAG => Ok($name::decode_exact(buf, size as _)?.into()),
                    )*
                    _ => Ok(Descriptor::Unknown(tag, Vec::decode_exact(buf, size as _)?)),
                }
            }
        }

        impl DecodeMaybe for Descriptor {
            fn decode_maybe<B: Buf>(buf: &mut B) -> Result<Option<Self>> {
                match buf.has_remaining() {
                    true => Descriptor::decode(buf).map(Some),
                    false => Ok(None),
                }
            }
        }

        impl Encode for Descriptor {
            fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
                // TODO This is inefficient; we could compute the size upfront.
                let mut tmp = Vec::new();

                match self {
                    $(
                        Descriptor::$name(t) => {
                            $name::TAG.encode(buf)?;
                            t.encode(&mut tmp)?;
                        },
                    )*
                    Descriptor::Unknown(tag, data) => {
                        tag.encode(buf)?;
                        data.encode(&mut tmp)?;
                    },
                };

                let mut size = tmp.len() as u32;
                while size > 0 {
                    let mut b = (size & 0x7F) as u8;
                    size >>= 7;
                    if size > 0 {
                        b |= 0x80;
                    }
                    b.encode(buf)?;
                }

                tmp.encode(buf)
            }
        }

        impl Descriptor {
            pub const fn tag(&self) -> u8 {
                match self {
                    $(
                        Descriptor::$name(_) => $name::TAG,
                    )*
                    Descriptor::Unknown(tag, _) => *tag,
                }
            }
        }

        $(
            impl From<$name> for Descriptor {
                fn from(desc: $name) -> Self {
                    Descriptor::$name(desc)
                }
            }
        )*
    };
}

descriptors! {
    EsDescriptor,
    DecoderConfig,
    DecoderSpecific,
    SLConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EsDescriptor {
    pub es_id: u16,

    pub dec_config: DecoderConfig,
    pub sl_config: SLConfig,
}

impl EsDescriptor {
    pub const TAG: u8 = 0x03;
}

impl Decode for EsDescriptor {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let es_id = u16::decode(buf)?;
        u8::decode(buf)?; // XXX flags must be 0

        let mut dec_config = None;
        let mut sl_config = None;

        while let Some(desc) = Descriptor::decode_maybe(buf)? {
            match desc {
                Descriptor::DecoderConfig(desc) => dec_config = Some(desc),
                Descriptor::SLConfig(desc) => sl_config = Some(desc),
                Descriptor::Unknown(tag, _) => tracing::warn!("unknown descriptor: {:02X}", tag),
                desc => return Err(Error::UnexpectedDescriptor(desc.tag())),
            }
        }

        Ok(EsDescriptor {
            es_id,
            dec_config: dec_config.ok_or(Error::MissingDescriptor(DecoderConfig::TAG))?,
            sl_config: sl_config.ok_or(Error::MissingDescriptor(SLConfig::TAG))?,
        })
    }
}

impl Encode for EsDescriptor {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.es_id.encode(buf)?;
        0u8.encode(buf)?;

        Descriptor::from(self.dec_config).encode(buf)?;
        Descriptor::from(self.sl_config).encode(buf)?;

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecoderConfig {
    pub object_type_indication: u8,
    pub stream_type: u8,
    pub up_stream: u8,
    pub buffer_size_db: u24,
    pub max_bitrate: u32,
    pub avg_bitrate: u32,
    pub dec_specific: DecoderSpecific,
}

impl DecoderConfig {
    pub const TAG: u8 = 0x04;
}

impl Decode for DecoderConfig {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let object_type_indication = u8::decode(buf)?;
        let byte_a = u8::decode(buf)?;
        let stream_type = (byte_a & 0xFC) >> 2;
        let up_stream = byte_a & 0x02;
        let buffer_size_db = u24::decode(buf)?;
        let max_bitrate = u32::decode(buf)?;
        let avg_bitrate = u32::decode(buf)?;

        let mut dec_specific = None;

        while let Some(desc) = Descriptor::decode_maybe(buf)? {
            match desc {
                Descriptor::DecoderSpecific(desc) => dec_specific = Some(desc),
                Descriptor::Unknown(tag, _) => tracing::warn!("unknown descriptor: {:02X}", tag),
                desc => return Err(Error::UnexpectedDescriptor(desc.tag())),
            }
        }

        Ok(DecoderConfig {
            object_type_indication,
            stream_type,
            up_stream,
            buffer_size_db,
            max_bitrate,
            avg_bitrate,
            dec_specific: dec_specific.ok_or(Error::MissingDescriptor(DecoderSpecific::TAG))?,
        })
    }
}

impl Encode for DecoderConfig {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.object_type_indication.encode(buf)?;
        ((self.stream_type << 2) + (self.up_stream & 0x02) + 1).encode(buf)?; // 1 reserved
        self.buffer_size_db.encode(buf)?;
        self.max_bitrate.encode(buf)?;
        self.avg_bitrate.encode(buf)?;

        Descriptor::from(self.dec_specific).encode(buf)?;

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DecoderSpecific {
    pub profile: u8,
    pub freq_index: u8,
    pub chan_conf: u8,
}

impl DecoderSpecific {
    pub const TAG: u8 = 0x05;
}

impl Decode for DecoderSpecific {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        let byte_a = u8::decode(buf)?;
        let byte_b = u8::decode(buf)?;

        let mut profile = byte_a >> 3;
        if profile == 31 {
            profile = 32 + ((byte_a & 7) | (byte_b >> 5));
        }

        let freq_index = if profile > 31 {
            (byte_b >> 1) & 0x0F
        } else {
            ((byte_a & 0x07) << 1) + (byte_b >> 7)
        };

        let chan_conf;
        if freq_index == 15 {
            // Skip the 24 bit sample rate
            // TODO this needs to be implemented in encode
            let sample_rate = u24::decode(buf)?;
            chan_conf = ((u32::from(sample_rate) >> 4) & 0x0F) as u8;
        } else if profile > 31 {
            let byte_c = u8::decode(buf)?;
            chan_conf = (byte_b & 1) | (byte_c & 0xE0);
        } else {
            chan_conf = (byte_b >> 3) & 0x0F;
        }

        if buf.has_remaining() {
            tracing::warn!("PLEASE FIX: failed to consume all bytes in DecoderSpecificDescriptor");
            buf.advance(buf.remaining());
        }

        Ok(DecoderSpecific {
            profile,
            freq_index,
            chan_conf,
        })
    }
}

impl Encode for DecoderSpecific {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        ((self.profile << 3) + (self.freq_index >> 1)).encode(buf)?;
        ((self.freq_index << 7) + (self.chan_conf << 3)).encode(buf)?;

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SLConfig {}

impl SLConfig {
    pub const TAG: u8 = 0x06;
}

impl Decode for SLConfig {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        u8::decode(buf)?; // pre-defined
        Ok(SLConfig {})
    }
}

impl Encode for SLConfig {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        2u8.encode(buf)?; // pre-defined
        Ok(())
    }
}
