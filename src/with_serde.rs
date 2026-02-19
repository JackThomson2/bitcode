use crate::coder::{Buffer, Decoder, Encoder, Result, View};
use crate::consume::consume_bytes;
use crate::length::{LengthDecoder, LengthEncoder};
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::num::NonZeroUsize;

pub struct WithSerdeEncoder<T: ?Sized> {
    lengths: LengthEncoder,
    data: Vec<u8>,
    _phantom: PhantomData<fn(T)>,
}

impl<T: ?Sized> Default for WithSerdeEncoder<T> {
    fn default() -> Self {
        Self {
            lengths: Default::default(),
            data: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<T: serde::Serialize + ?Sized> Encoder<T> for WithSerdeEncoder<T> {
    fn encode(&mut self, t: &T) {
        let bytes = crate::serde::serialize(t).expect("with_serde: serialize failed");
        self.lengths.encode(&bytes.len());
        self.data.extend_from_slice(&bytes);
    }
}

impl<T: ?Sized> Buffer for WithSerdeEncoder<T> {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        self.lengths.collect_into(out);
        out.extend_from_slice(&self.data);
        self.data.clear();
    }
    fn reserve(&mut self, additional: NonZeroUsize) {
        self.lengths.reserve(additional);
    }
}

pub struct WithSerdeDecoder<'a, T> {
    lengths: LengthDecoder<'a>,
    data: &'a [u8],
    _phantom: PhantomData<fn() -> T>,
}

impl<T> Default for WithSerdeDecoder<'_, T> {
    fn default() -> Self {
        Self {
            lengths: Default::default(),
            data: &[],
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> View<'a> for WithSerdeDecoder<'a, T> {
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()> {
        self.lengths.populate(input, length)?;
        self.data = consume_bytes(input, self.lengths.length())?;
        Ok(())
    }
}

impl<'a, T: serde::de::DeserializeOwned> Decoder<'a, T> for WithSerdeDecoder<'a, T> {
    fn decode(&mut self) -> T {
        let len = self.lengths.decode();
        let (bytes, rest) = self.data.split_at(len);
        self.data = rest;
        crate::serde::deserialize(bytes).expect("with_serde: deserialize failed")
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
    struct SerdeOnly {
        name: String,
        value: i64,
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    struct Mixed {
        x: u32,
        #[bitcode(with_serde)]
        y: SerdeOnly,
    }

    #[test]
    fn with_serde_roundtrip() {
        let original = Mixed {
            x: 42,
            y: SerdeOnly {
                name: "hello".into(),
                value: -7,
            },
        };
        let encoded = crate::encode(&original);
        let decoded: Mixed = crate::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    enum MixedEnum {
        A(u32),
        B {
            #[bitcode(with_serde)]
            s: SerdeOnly,
        },
    }

    #[test]
    fn with_serde_enum_variant() {
        let original = MixedEnum::B {
            s: SerdeOnly {
                name: "enum".into(),
                value: 99,
            },
        };
        let encoded = crate::encode(&original);
        let decoded: MixedEnum = crate::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    struct MultiSerde {
        #[bitcode(with_serde)]
        a: SerdeOnly,
        x: u8,
        #[bitcode(with_serde)]
        b: SerdeOnly,
    }

    #[test]
    fn with_serde_multiple_fields() {
        let original = MultiSerde {
            a: SerdeOnly { name: "first".into(), value: 1 },
            x: 255,
            b: SerdeOnly { name: "second".into(), value: 2 },
        };
        let encoded = crate::encode(&original);
        let decoded: MultiSerde = crate::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    struct WithSkipAndSerde {
        #[bitcode(skip)]
        skipped: u32,
        #[bitcode(with_serde)]
        serde_field: SerdeOnly,
    }

    #[test]
    fn with_serde_alongside_skip() {
        let original = WithSkipAndSerde {
            skipped: 999,
            serde_field: SerdeOnly { name: "kept".into(), value: 42 },
        };
        let encoded = crate::encode(&original);
        let decoded: WithSkipAndSerde = crate::decode(&encoded).unwrap();
        assert_eq!(decoded.skipped, 0); // skipped field defaults
        assert_eq!(decoded.serde_field, original.serde_field);
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    struct VecOfSerde {
        #[bitcode(with_serde)]
        items: Vec<SerdeOnly>,
    }

    #[test]
    fn with_serde_collection() {
        let original = VecOfSerde {
            items: vec![
                SerdeOnly { name: "a".into(), value: 1 },
                SerdeOnly { name: "b".into(), value: 2 },
            ],
        };
        let encoded = crate::encode(&original);
        let decoded: VecOfSerde = crate::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    struct TupleSerde(u32, #[bitcode(with_serde)] SerdeOnly);

    #[test]
    fn with_serde_tuple_struct() {
        let original = TupleSerde(7, SerdeOnly { name: "tuple".into(), value: -1 });
        let encoded = crate::encode(&original);
        let decoded: TupleSerde = crate::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }
}
