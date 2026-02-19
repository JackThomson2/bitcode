use crate::coder::{Buffer, Decoder, Encoder, Result, View};
use crate::consume::consume_bytes;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::num::NonZeroUsize;

/// Trait for types that can be safely read as raw bytes.
///
/// # Safety
/// The type must have a fixed layout with no padding-dependent invariants.
/// All bit patterns within the byte representation must be valid.
pub unsafe trait AsBytes {
    fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(self as *const Self as *const u8, core::mem::size_of_val(self))
        }
    }
}

/// Trait for types that can be safely constructed from raw bytes.
///
/// # Safety
/// The type must accept any bit pattern of the correct length as valid.
pub unsafe trait FromBytes: Sized {
    fn from_bytes(bytes: &[u8]) -> Self {
        assert!(bytes.len() >= core::mem::size_of::<Self>());
        unsafe { core::ptr::read(bytes.as_ptr() as *const Self) }
    }
}

/// Encoder for POD types — writes raw bytes. No serde, no framing overhead.
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

impl<T: AsBytes> Encoder<T> for WithBytesEncoder<T> {
    #[inline(always)]
    fn encode(&mut self, t: &T) {
        self.data.extend_from_slice(t.as_bytes());
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

/// Decoder for POD types — zero-copy read from input buffer.
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

impl<'a, T: FromBytes> Decoder<'a, T> for WithBytesDecoder<'a, T> {
    #[inline(always)]
    fn decode(&mut self) -> T {
        let size = core::mem::size_of::<T>();
        let (bytes, rest) = self.data.split_at(size);
        self.data = rest;
        T::from_bytes(bytes)
    }
}

// Blanket impl for bytemuck types
unsafe impl<T: bytemuck::NoUninit> AsBytes for T {}
unsafe impl<T: bytemuck::AnyBitPattern> FromBytes for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
    #[repr(C)]
    struct PodType {
        a: u32,
        b: u32,
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
