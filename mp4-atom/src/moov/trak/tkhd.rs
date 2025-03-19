use crate::*;

ext! {
    name: Tkhd,
    versions: [0, 1],
    flags: {
        track_enabled = 0,
        track_in_movie = 1,
        track_in_preview = 2,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Tkhd {
    pub creation_time: u64,
    pub modification_time: u64,
    pub track_id: u32,
    pub duration: u64,
    pub layer: u16,
    pub alternate_group: u16,
    pub enabled: bool,

    pub volume: FixedPoint<u8>,
    pub matrix: Matrix,

    pub width: FixedPoint<u16>,
    pub height: FixedPoint<u16>,
}

impl AtomExt for Tkhd {
    const KIND_EXT: FourCC = FourCC::new(b"tkhd");

    type Ext = TkhdExt;

    fn decode_body_ext<B: Buf>(buf: &mut B, ext: TkhdExt) -> Result<Self> {
        let (creation_time, modification_time, track_id, _, duration) = match ext.version {
            TkhdVersion::V1 => (
                u64::decode(buf)?,
                u64::decode(buf)?,
                u32::decode(buf)?,
                u32::decode(buf)?,
                u64::decode(buf)?,
            ),
            TkhdVersion::V0 => (
                u32::decode(buf)? as u64,
                u32::decode(buf)? as u64,
                u32::decode(buf)?,
                u32::decode(buf)?,
                u32::decode(buf)? as u64,
            ),
        };

        u64::decode(buf)?; // reserved
        let layer = u16::decode(buf)?;
        let alternate_group = u16::decode(buf)?;
        let volume = FixedPoint::decode(buf)?;

        u16::decode(buf)?; // reserved
        let matrix = Matrix::decode(buf)?;
        let width = FixedPoint::decode(buf)?;
        let height = FixedPoint::decode(buf)?;

        Ok(Tkhd {
            creation_time,
            modification_time,
            track_id,
            duration,
            layer,
            alternate_group,
            volume,
            matrix,
            width,
            height,
            enabled: ext.track_enabled,
        })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<TkhdExt> {
        self.creation_time.encode(buf)?;
        self.modification_time.encode(buf)?;
        self.track_id.encode(buf)?;
        0u32.encode(buf)?; // reserved
        self.duration.encode(buf)?;

        0u64.encode(buf)?; // reserved
        self.layer.encode(buf)?;
        self.alternate_group.encode(buf)?;
        self.volume.encode(buf)?;
        0u16.encode(buf)?; // reserved
        self.matrix.encode(buf)?;

        self.width.encode(buf)?;
        self.height.encode(buf)?;

        Ok(TkhdExt {
            version: TkhdVersion::V1,
            track_enabled: self.enabled,
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Matrix {
    pub a: i32,
    pub b: i32,
    pub u: i32,
    pub c: i32,
    pub d: i32,
    pub v: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
}

impl Decode for Matrix {
    fn decode<B: Buf>(buf: &mut B) -> Result<Self> {
        Ok(Self {
            a: i32::decode(buf)?,
            b: i32::decode(buf)?,
            u: i32::decode(buf)?,
            c: i32::decode(buf)?,
            d: i32::decode(buf)?,
            v: i32::decode(buf)?,
            x: i32::decode(buf)?,
            y: i32::decode(buf)?,
            w: i32::decode(buf)?,
        })
    }
}

impl Encode for Matrix {
    fn encode<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        self.a.encode(buf)?;
        self.b.encode(buf)?;
        self.u.encode(buf)?;
        self.c.encode(buf)?;
        self.d.encode(buf)?;
        self.v.encode(buf)?;
        self.x.encode(buf)?;
        self.y.encode(buf)?;
        self.w.encode(buf)
    }
}

impl std::fmt::Display for Matrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x} {:#x}",
            self.a, self.b, self.u, self.c, self.d, self.v, self.x, self.y, self.w
        )
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Self {
            // unity matrix according to ISO/IEC 14496-12:2005(E)
            a: 0x00010000,
            b: 0,
            u: 0,
            c: 0,
            d: 0x00010000,
            v: 0,
            x: 0,
            y: 0,
            w: 0x40000000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tkhd32() {
        let expected = Tkhd {
            creation_time: 100,
            modification_time: 200,
            track_id: 1,
            duration: 634634,
            layer: 0,
            alternate_group: 0,
            volume: 1.into(),
            matrix: Matrix::default(),
            width: 512.into(),
            height: 288.into(),
            enabled: true,
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Tkhd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_tkhd64() {
        let expected = Tkhd {
            creation_time: 100,
            modification_time: 200,
            track_id: 1,
            duration: 634634,
            layer: 0,
            alternate_group: 0,
            volume: 1.into(),
            matrix: Matrix::default(),
            width: 512.into(),
            height: 288.into(),
            enabled: true,
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Tkhd::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
