use crate::*;

ext! {
    name: Emsg,
    versions: [0, 1],
    flags: {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum EmsgTimestamp {
    Relative(u32),
    Absolute(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Emsg {
    pub timescale: u32,
    pub presentation_time: EmsgTimestamp,
    pub event_duration: u32,
    pub id: u32,
    pub scheme_id_uri: String,
    pub value: String,
    pub message_data: Vec<u8>,
}

impl AtomExt for Emsg {
    const KIND_EXT: FourCC = FourCC::new(b"emsg");

    type Ext = EmsgExt;

    fn decode_body_ext<B: Buf>(buf: &mut B, ext: EmsgExt) -> Result<Self> {
        Ok(match ext.version {
            EmsgVersion::V0 => Emsg {
                scheme_id_uri: String::decode(buf)?,
                value: String::decode(buf)?,
                timescale: u32::decode(buf)?,
                presentation_time: EmsgTimestamp::Relative(u32::decode(buf)?),
                event_duration: u32::decode(buf)?,
                id: u32::decode(buf)?,
                message_data: Vec::decode(buf)?,
            },
            EmsgVersion::V1 => Emsg {
                timescale: u32::decode(buf)?,
                presentation_time: EmsgTimestamp::Absolute(u64::decode(buf)?),
                event_duration: u32::decode(buf)?,
                id: u32::decode(buf)?,
                scheme_id_uri: String::decode(buf)?,
                value: String::decode(buf)?,
                message_data: Vec::decode(buf)?,
            },
        })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<EmsgExt> {
        Ok(match self.presentation_time {
            EmsgTimestamp::Absolute(presentation_time) => {
                self.timescale.encode(buf)?;
                presentation_time.encode(buf)?;
                self.event_duration.encode(buf)?;
                self.id.encode(buf)?;
                self.scheme_id_uri.as_str().encode(buf)?;
                self.value.as_str().encode(buf)?;
                self.message_data.encode(buf)?;

                EmsgVersion::V1.into()
            }
            EmsgTimestamp::Relative(presentation_time) => {
                self.scheme_id_uri.as_str().encode(buf)?;
                self.value.as_str().encode(buf)?;
                self.timescale.encode(buf)?;
                presentation_time.encode(buf)?;
                self.event_duration.encode(buf)?;
                self.id.encode(buf)?;
                self.message_data.encode(buf)?;

                EmsgVersion::V0.into()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emsg_version0() {
        let decoded = Emsg {
            timescale: 48000,
            event_duration: 200,
            presentation_time: EmsgTimestamp::Relative(100),
            id: 8,
            scheme_id_uri: String::from("foo"),
            value: String::from("foo"),
            message_data: [1, 2, 3].into(),
        };

        let mut buf = Vec::new();
        decoded.encode(&mut buf).unwrap();

        let output = Emsg::decode(&mut buf.as_slice()).unwrap();
        assert_eq!(decoded, output);
    }

    #[test]
    fn test_emsg_version1() {
        let decoded = Emsg {
            presentation_time: EmsgTimestamp::Absolute(50000),
            timescale: 48000,
            event_duration: 200,
            id: 8,
            scheme_id_uri: String::from("foo"),
            value: String::from("foo"),
            message_data: [3, 2, 1].into(),
        };

        let mut buf = Vec::new();
        decoded.encode(&mut buf).unwrap();

        let output = Emsg::decode(&mut buf.as_slice()).unwrap();
        assert_eq!(decoded, output);
    }
}
