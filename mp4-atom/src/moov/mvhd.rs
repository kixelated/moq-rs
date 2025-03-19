use crate::*;

ext! {
    name: Mvhd,
    versions: [0,1],
    flags: {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mvhd {
    pub creation_time: u64,
    pub modification_time: u64,
    pub timescale: u32,
    pub duration: u64,

    pub rate: FixedPoint<u16>,
    pub volume: FixedPoint<u8>,

    pub matrix: Matrix,
    pub next_track_id: u32,
}

impl AtomExt for Mvhd {
    const KIND_EXT: FourCC = FourCC::new(b"mvhd");

    type Ext = MvhdExt;

    fn decode_body_ext<B: Buf>(buf: &mut B, ext: MvhdExt) -> Result<Self> {
        let (creation_time, modification_time, timescale, duration) = match ext.version {
            MvhdVersion::V1 => (
                u64::decode(buf)?,
                u64::decode(buf)?,
                u32::decode(buf)?,
                u64::decode(buf)?,
            ),
            MvhdVersion::V0 => (
                u32::decode(buf)? as u64,
                u32::decode(buf)? as u64,
                u32::decode(buf)?,
                u32::decode(buf)? as u64,
            ),
        };

        let rate = FixedPoint::decode(buf)?;
        let volume = FixedPoint::decode(buf)?;

        u16::decode(buf)?; // reserved
        u64::decode(buf)?; // reserved

        let matrix = Matrix::decode(buf)?;

        <[u8; 24]>::decode(buf)?; // pre_defined = 0

        let next_track_id = u32::decode(buf)?;

        Ok(Mvhd {
            creation_time,
            modification_time,
            timescale,
            duration,
            rate,
            volume,
            matrix,
            next_track_id,
        })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<MvhdExt> {
        self.creation_time.encode(buf)?;
        self.modification_time.encode(buf)?;
        self.timescale.encode(buf)?;
        self.duration.encode(buf)?;

        self.rate.encode(buf)?;
        self.volume.encode(buf)?;

        0u16.encode(buf)?; // reserved
        0u64.encode(buf)?; // reserved

        self.matrix.encode(buf)?;

        [0u8; 24].encode(buf)?; // pre_defined = 0

        self.next_track_id.encode(buf)?;

        Ok(MvhdVersion::V1.into())
    }
}

impl Default for Mvhd {
    fn default() -> Self {
        Mvhd {
            creation_time: 0,
            modification_time: 0,
            timescale: 1000,
            duration: 0,
            rate: Default::default(),
            matrix: Default::default(),
            volume: Default::default(),
            next_track_id: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mvhd32() {
        let expected = Mvhd {
            creation_time: 100,
            modification_time: 200,
            timescale: 1000,
            duration: 634634,
            rate: 1.into(),
            volume: 1.into(),
            matrix: Matrix::default(),
            next_track_id: 1,
        };

        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Mvhd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_mvhd64() {
        let expected = Mvhd {
            creation_time: 100,
            modification_time: 200,
            timescale: 1000,
            duration: 634634,
            rate: 1.into(),
            volume: 1.into(),
            matrix: Matrix::default(),
            next_track_id: 1,
        };

        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let output = Mvhd::decode(&mut buf).unwrap();
        assert_eq!(output, expected);
    }
}
