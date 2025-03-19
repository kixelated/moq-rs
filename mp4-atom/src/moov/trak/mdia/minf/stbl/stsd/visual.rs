use crate::coding::{Decode, Encode};
use crate::{Buf, BufMut, Compressor, FixedPoint, Result};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Visual {
    pub data_reference_index: u16,
    pub width: u16,
    pub height: u16,
    pub horizresolution: FixedPoint<u16>,
    pub vertresolution: FixedPoint<u16>,
    pub frame_count: u16,
    pub compressor: Compressor,
    pub depth: u16,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            data_reference_index: 0,
            width: 0,
            height: 0,
            horizresolution: 0x48.into(),
            vertresolution: 0x48.into(),
            frame_count: 1,
            compressor: Default::default(),
            depth: 0x0018,
        }
    }
}

impl Encode for Visual {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        0u32.encode(buf)?; // reserved
        0u16.encode(buf)?; // reserved
        self.data_reference_index.encode(buf)?;

        0u32.encode(buf)?; // pre-defined, reserved
        0u64.encode(buf)?; // pre-defined
        0u32.encode(buf)?; // pre-defined
        self.width.encode(buf)?;
        self.height.encode(buf)?;
        self.horizresolution.encode(buf)?;
        self.vertresolution.encode(buf)?;
        0u32.encode(buf)?; // reserved
        self.frame_count.encode(buf)?;
        self.compressor.encode(buf)?;
        self.depth.encode(buf)?;
        (-1i16).encode(buf)?; // pre-defined

        Ok(())
    }
}
impl Decode for Visual {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        /*
        class VisualSampleEntry(codingname) extends SampleEntry (codingname){
            unsigned int(16) pre_defined = 0;
            const unsigned int(16) reserved = 0;
            unsigned int(32)[3]  pre_defined = 0;
            unsigned int(16)  width;
            unsigned int(16)  height;
            template unsigned int(32)  horizresolution = 0x00480000; // 72 dpi
            template unsigned int(32)  vertresolution  = 0x00480000; // 72 dpi
            const unsigned int(32)  reserved = 0;
            template unsigned int(16)  frame_count = 1;
            string[32]  compressorname;
            template unsigned int(16)  depth = 0x0018;
            int(16)  pre_defined = -1;
            // other boxes from derived specifications
            CleanApertureBox     clap;    // optional
            PixelAspectRatioBox  pasp;    // optional
        }

         */

        // SampleEntry
        <[u8; 6]>::decode(buf)?;
        let data_reference_index = u16::decode(buf)?;

        // VisualSampleEntry
        // 16 bytes of garb at the front
        <[u8; 16]>::decode(buf)?;
        let width = u16::decode(buf)?;
        let height = u16::decode(buf)?;
        let horizresolution = FixedPoint::decode(buf)?;
        let vertresolution = FixedPoint::decode(buf)?;
        u32::decode(buf)?; // reserved
        let frame_count = u16::decode(buf)?;
        let compressor = Compressor::decode(buf)?;
        let depth = u16::decode(buf)?;
        i16::decode(buf)?; // pre-defined

        Ok(Self {
            data_reference_index,
            width,
            height,
            horizresolution,
            vertresolution,
            frame_count,
            compressor,
            depth,
        })
    }
}
