use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Avcc {
    pub configuration_version: u8,
    pub avc_profile_indication: u8,
    pub profile_compatibility: u8,
    pub avc_level_indication: u8,
    pub length_size: u8,
    pub sequence_parameter_sets: Vec<Vec<u8>>,
    pub picture_parameter_sets: Vec<Vec<u8>>,
    pub ext: Option<AvccExt>,
}

// Only valid for certain profiles
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AvccExt {
    pub chroma_format: u8,
    pub bit_depth_luma: u8,
    pub bit_depth_chroma: u8,
    pub sequence_parameter_sets_ext: Vec<Vec<u8>>,
}

impl Default for AvccExt {
    fn default() -> Self {
        AvccExt {
            chroma_format: 1,
            bit_depth_luma: 8,
            bit_depth_chroma: 8,
            sequence_parameter_sets_ext: Vec::new(),
        }
    }
}

impl Avcc {
    pub fn new(sps: &[u8], pps: &[u8]) -> Result<Self> {
        if sps.len() < 4 {
            return Err(Error::OutOfBounds);
        }

        Ok(Self {
            configuration_version: 1,
            avc_profile_indication: sps[1],
            profile_compatibility: sps[2],
            avc_level_indication: sps[3],
            length_size: 4,
            sequence_parameter_sets: vec![sps.into()],
            picture_parameter_sets: vec![pps.into()],

            // TODO This information could be parsed out of the SPS
            ext: None,
        })
    }
}

impl Atom for Avcc {
    const KIND: FourCC = FourCC::new(b"avcC");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        // print the buffer
        tracing::info!("{:02X?}", buf);

        let configuration_version = u8::decode(buf)?;
        if configuration_version != 1 {
            return Err(Error::UnknownVersion(configuration_version));
        }
        let avc_profile_indication = u8::decode(buf)?;
        let profile_compatibility = u8::decode(buf)?;
        let avc_level_indication = u8::decode(buf)?;

        // The first 5 bits are reserved as 0b11111 and the value is encoded -1
        let mut length_size = u8::decode(buf)?;
        length_size = match length_size {
            0xfc..=0xff => (length_size & 0x03) + 1,
            _ => return Err(Error::InvalidSize),
        };

        let num_of_spss = u8::decode(buf)? & 0x1F;
        let mut sequence_parameter_sets = Vec::with_capacity(num_of_spss as usize);
        for _ in 0..num_of_spss {
            let size = u16::decode(buf)? as usize;
            let nal = Vec::decode_exact(buf, size)?;
            sequence_parameter_sets.push(nal);
        }

        let num_of_ppss = u8::decode(buf)?;
        let mut picture_parameter_sets = Vec::with_capacity(num_of_ppss as usize);
        for _ in 0..num_of_ppss {
            let size = u16::decode(buf)? as usize;
            let nal = Vec::decode_exact(buf, size)?;
            picture_parameter_sets.push(nal);
        }

        let ext = match avc_profile_indication {
            // NOTE: Many encoders/decoders skip this part, so it's not always present
            100 | 110 | 122 | 144 if buf.remaining() > 0 => {
                let chroma_format = u8::decode(buf)? & 0x3;
                let bit_depth_luma_minus8 = u8::decode(buf)? & 0x7;
                let bit_depth_chroma_minus8 = u8::decode(buf)? & 0x7;
                let num_of_sequence_parameter_set_exts = u8::decode(buf)? as usize;
                let mut sequence_parameter_sets_ext =
                    Vec::with_capacity(num_of_sequence_parameter_set_exts);

                for _ in 0..num_of_sequence_parameter_set_exts {
                    let size = u16::decode(buf)? as usize;
                    let nal = Vec::decode_exact(buf, size)?;
                    sequence_parameter_sets_ext.push(nal);
                }

                Some(AvccExt {
                    chroma_format,
                    bit_depth_luma: bit_depth_luma_minus8 + 8,
                    bit_depth_chroma: bit_depth_chroma_minus8 + 8,
                    sequence_parameter_sets_ext,
                })
            }
            _ => None,
        };

        // print everything
        tracing::info!("configuration_version: {:?}", configuration_version);
        tracing::info!("avc_profile_indication: {:?}", avc_profile_indication);
        tracing::info!("profile_compatibility: {:?}", profile_compatibility);
        tracing::info!("avc_level_indication: {:?}", avc_level_indication);
        tracing::info!("length_size: {:?}", length_size);
        tracing::info!("sequence_parameter_sets: {:02X?}", sequence_parameter_sets);
        tracing::info!("picture_parameter_sets: {:02X?}", picture_parameter_sets);
        tracing::info!("ext: {:?}", ext);

        tracing::info!("{:02X?}", buf);

        Ok(Avcc {
            configuration_version,
            avc_profile_indication,
            profile_compatibility,
            avc_level_indication,
            length_size,
            sequence_parameter_sets,
            picture_parameter_sets,
            ext,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.configuration_version.encode(buf)?;
        self.avc_profile_indication.encode(buf)?;
        self.profile_compatibility.encode(buf)?;
        self.avc_level_indication.encode(buf)?;
        let length_size = match self.length_size {
            0 => return Err(Error::InvalidSize),
            1..=4 => self.length_size - 1,
            _ => return Err(Error::InvalidSize),
        };
        (length_size | 0xFC).encode(buf)?;

        (self.sequence_parameter_sets.len() as u8 | 0xE0).encode(buf)?;
        for sps in &self.sequence_parameter_sets {
            (sps.len() as u16).encode(buf)?;
            sps.encode(buf)?;
        }

        (self.picture_parameter_sets.len() as u8).encode(buf)?;
        for pps in &self.picture_parameter_sets {
            (pps.len() as u16).encode(buf)?;
            pps.encode(buf)?;
        }

        if let Some(ext) = &self.ext {
            ext.chroma_format.encode(buf)?;
            ext.bit_depth_luma.encode(buf)?;
            ext.bit_depth_chroma.encode(buf)?;
            (ext.sequence_parameter_sets_ext.len() as u8).encode(buf)?;
            for sps in &ext.sequence_parameter_sets_ext {
                (sps.len() as u16).encode(buf)?;
                sps.encode(buf)?;
            }
        }

        Ok(())
    }
}
