use crate::coding::{Decode, Encode};
use crate::{Any, Atom, Buf, BufMut, DecodeMaybe, Error, FourCC, Result};

use super::Visual;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Av01 {
    pub visual: Visual,
    pub av1c: Av1c,
}

impl Atom for Av01 {
    const KIND: FourCC = FourCC::new(b"av01");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        let visual = Visual::decode(buf)?;

        let mut av1c = None;
        while let Some(atom) = Any::decode_maybe(buf)? {
            match atom {
                Any::Av1c(atom) => av1c = atom.into(),
                _ => tracing::warn!("unknown atom: {:?}", atom),
            }
        }

        Ok(Av01 {
            visual,
            av1c: av1c.ok_or(Error::MissingBox(Av1c::KIND))?,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.visual.encode(buf)?;
        self.av1c.encode(buf)?;

        Ok(())
    }
}

// https://aomediacodec.github.io/av1-isobmff/#av1codecconfigurationbox-section
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Av1c {
    pub seq_profile: u8,
    pub seq_level_idx_0: u8,
    pub seq_tier_0: bool,
    pub high_bitdepth: bool,
    pub twelve_bit: bool,
    pub monochrome: bool,
    pub chroma_subsampling_x: bool,
    pub chroma_subsampling_y: bool,
    pub chroma_sample_position: u8, // 0..3
    pub initial_presentation_delay: Option<u8>,
    pub config_obus: Vec<u8>,
}

impl Atom for Av1c {
    const KIND: FourCC = FourCC::new(b"av1C");

    fn decode_body<B: Buf>(buf: &mut B) -> Result<Self> {
        let version = u8::decode(buf)?;
        if version != 0b1000_0001 {
            return Err(Error::UnknownVersion(version));
        }

        let v = u8::decode(buf)?;
        let seq_profile = v >> 5;
        let seq_level_idx_0 = v & 0b11111;

        let v = u8::decode(buf)?;
        let seq_tier_0 = (v >> 7) == 1;
        let high_bitdepth = ((v >> 6) & 0b1) == 1;
        let twelve_bit = ((v >> 5) & 0b1) == 1;
        let monochrome = ((v >> 4) & 0b1) == 1;
        let chroma_subsampling_x = ((v >> 3) & 0b1) == 1;
        let chroma_subsampling_y = ((v >> 2) & 0b1) == 1;
        let chroma_sample_position = v & 0b11;

        let v = u8::decode(buf)?;
        let reserved = v >> 5;
        if reserved != 0 {
            return Err(Error::Reserved);
        }

        let initial_presentation_delay_present = (v >> 4) & 0b1;
        let initial_presentation_delay_minus_one = v & 0b1111;

        let initial_presentation_delay = if initial_presentation_delay_present == 1 {
            Some(initial_presentation_delay_minus_one + 1)
        } else {
            if initial_presentation_delay_minus_one != 0 {
                return Err(Error::Reserved);
            }

            None
        };

        let config_obus = Vec::decode(buf)?;

        Ok(Self {
            seq_profile,
            seq_level_idx_0,
            seq_tier_0,
            high_bitdepth,
            twelve_bit,
            monochrome,
            chroma_subsampling_x,
            chroma_subsampling_y,
            chroma_sample_position,
            initial_presentation_delay,
            config_obus,
        })
    }

    fn encode_body<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        0b1000_0001_u8.encode(buf)?;
        ((self.seq_profile << 5) | self.seq_level_idx_0).encode(buf)?;

        (((self.seq_tier_0 as u8) << 7)
            | ((self.high_bitdepth as u8) << 6)
            | ((self.twelve_bit as u8) << 5)
            | ((self.monochrome as u8) << 4)
            | ((self.chroma_subsampling_x as u8) << 3)
            | ((self.chroma_subsampling_y as u8) << 2)
            | self.chroma_sample_position)
            .encode(buf)?;

        if let Some(initial_presentation_delay) = self.initial_presentation_delay {
            ((initial_presentation_delay - 1) | 0b0001_0000).encode(buf)?;
        } else {
            0b0000_0000_u8.encode(buf)?;
        }

        self.config_obus.encode(buf)?;

        Ok(())
    }
}
