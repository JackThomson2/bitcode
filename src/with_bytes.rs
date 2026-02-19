use crate::coder::{Buffer, Decoder, Encoder, Result, View};
use crate::consume::consume_bytes;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::num::NonZeroUsize;

/// Encoder for POD types — writes raw bytes via memcpy. No serde, no framing.
///
/// # Safety
/// The user asserts via `#[bitcode(with_bytes)]` that `T` is a plain-old-data type
/// safe to transmit as raw bytes (no padding-dependent invariants, repr(C), etc.).
pub struct WithBytesEncoder<T> {
    data: Vec<u8>,
    _phantom: PhantomData<fn(T)>,
}

impl<T> Default for WithBytesEncoder<T> {
    fn default() -> Self {
        Self {
            data: Vec::new(),
            _phantom: PhantomData,
        }
    }
}

impl<T: Copy> Encoder<T> for WithBytesEncoder<T> {
    #[inline(always)]
    fn encode(&mut self, t: &T) {
        let bytes = unsafe {
            core::slice::from_raw_parts(t as *const T as *const u8, core::mem::size_of::<T>())
        };
        self.data.extend_from_slice(bytes);
    }
}

impl<T> Buffer for WithBytesEncoder<T> {
    fn collect_into(&mut self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.data);
        self.data.clear();
    }
    fn reserve(&mut self, additional: NonZeroUsize) {
        self.data
            .reserve(additional.get() * core::mem::size_of::<T>());
    }
}

/// Decoder for POD types — reads raw bytes via memcpy. Zero-copy from input.
pub struct WithBytesDecoder<'a, T> {
    data: &'a [u8],
    _phantom: PhantomData<fn() -> T>,
}

impl<T> Default for WithBytesDecoder<'_, T> {
    fn default() -> Self {
        Self {
            data: &[],
            _phantom: PhantomData,
        }
    }
}

impl<'a, T> View<'a> for WithBytesDecoder<'a, T> {
    fn populate(&mut self, input: &mut &'a [u8], length: usize) -> Result<()> {
        let total = length
            .checked_mul(core::mem::size_of::<T>())
            .ok_or_else(|| crate::error::error("length overflow"))?;
        self.data = consume_bytes(input, total)?;
        Ok(())
    }
}

impl<'a, T: Copy> Decoder<'a, T> for WithBytesDecoder<'a, T> {
    #[inline(always)]
    fn decode(&mut self) -> T {
        let size = core::mem::size_of::<T>();
        debug_assert!(self.data.len() >= size);
        let (bytes, rest) = self.data.split_at(size);
        self.data = rest;
        unsafe { core::ptr::read(bytes.as_ptr() as *const T) }
    }
}

#[cfg(test)]
mod tests {
    #[derive(Copy, Clone, Debug, Default, PartialEq)]
    #[repr(C)]
    struct PodType {
        a: u32,
        b: u64,
        c: [u8; 16],
    }

    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    struct WithPod {
        x: u32,
        #[bitcode(with_bytes)]
        pod: PodType,
        y: bool,
    }

    #[test]
    fn with_bytes_roundtrip() {
        let original = WithPod {
            x: 42,
            pod: PodType {
                a: 0xDEAD,
                b: 0xBEEF_CAFE,
                c: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            },
            y: true,
        };
        let encoded = crate::encode(&original);
        let decoded: WithPod = crate::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn with_bytes_vec_of_structs() {
        #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
        struct Container {
            items: Vec<Item>,
        }

        #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
        struct Item {
            id: u32,
            #[bitcode(with_bytes)]
            data: PodType,
        }

        let original = Container {
            items: vec![
                Item { id: 1, data: PodType { a: 10, b: 20, c: [0; 16] } },
                Item { id: 2, data: PodType { a: 30, b: 40, c: [0xFF; 16] } },
            ],
        };
        let encoded = crate::encode(&original);
        let decoded: Container = crate::decode(&encoded).unwrap();
        assert_eq!(original, decoded);
    }
}
