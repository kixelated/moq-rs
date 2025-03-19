use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StszSamples {
    Identical { count: u32, size: u32 },
    Different { sizes: Vec<u32> },
}

impl Default for StszSamples {
    fn default() -> Self {
        StszSamples::Different { sizes: vec![] }
    }
}

/// Sample Size Box (stsz)
///
/// Lists the size of each sample in the track.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stsz {
    pub samples: StszSamples,
}

impl AtomExt for Stsz {
    type Ext = ();

    const KIND_EXT: FourCC = FourCC::new(b"stsz");

    fn decode_body_ext<B: Buf>(buf: &mut B, _ext: ()) -> Result<Self> {
        let size = u32::decode(buf)?;
        let count = u32::decode(buf)?;

        let samples = match size {
            0 => {
                let mut sizes = Vec::with_capacity(count.min(1024) as usize);
                for _ in 0..count {
                    sizes.push(u32::decode(buf)?)
                }
                StszSamples::Different { sizes }
            }
            size => StszSamples::Identical { count, size },
        };

        Ok(Stsz { samples })
    }

    fn encode_body_ext<B: BufMut>(&self, buf: &mut B) -> Result<()> {
        match self.samples {
            StszSamples::Identical { count, size } => {
                size.encode(buf)?;
                count.encode(buf)?;
            }
            StszSamples::Different { ref sizes } => {
                0u32.encode(buf)?;
                (sizes.len() as u32).encode(buf)?;

                for size in sizes {
                    size.encode(buf)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stsz_same_size() {
        let expected = Stsz {
            samples: StszSamples::Identical {
                count: 4,
                size: 1165,
            },
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Stsz::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_stsz_many_sizes() {
        let expected = Stsz {
            samples: StszSamples::Different {
                sizes: vec![1165, 11, 11, 8545, 10126, 10866, 9643, 9351, 7730],
            },
        };
        let mut buf = Vec::new();
        expected.encode(&mut buf).unwrap();

        let mut buf = buf.as_ref();
        let decoded = Stsz::decode(&mut buf).unwrap();
        assert_eq!(decoded, expected);
    }
}
