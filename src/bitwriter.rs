/// MSB-first bit writer for ALAC frame output.
///
/// Writes bits into a byte buffer, accumulating in a u64 shift register
/// for efficiency. Flushes complete bytes as they fill.
pub struct BitWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,   // byte position in buf
    bit_buf: u64, // accumulated bits (MSB-first)
    bit_pos: u32, // number of valid bits in bit_buf (0..64)
}

impl<'a> BitWriter<'a> {
    /// Create a new bit writer over the given output buffer.
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self {
            buf,
            pos: 0,
            bit_buf: 0,
            bit_pos: 0,
        }
    }

    /// Write `nbits` (1..=32) from the low bits of `val`, MSB-first.
    #[inline]
    pub fn write(&mut self, val: u32, nbits: u32) {
        debug_assert!(nbits > 0 && nbits <= 32);
        debug_assert!(self.bit_pos + nbits <= 64);

        // Shift val into the accumulator, MSB-aligned.
        self.bit_buf |= (val as u64 & ((1u64 << nbits) - 1)) << (64 - self.bit_pos - nbits);
        self.bit_pos += nbits;

        // Flush complete bytes.
        while self.bit_pos >= 8 {
            self.buf[self.pos] = (self.bit_buf >> 56) as u8;
            self.pos += 1;
            self.bit_buf <<= 8;
            self.bit_pos -= 8;
        }
    }

    /// Write a single bit.
    #[allow(dead_code)]
    #[inline]
    pub fn write_bit(&mut self, bit: u32) {
        self.write(bit & 1, 1);
    }

    /// Flush any remaining partial byte and return the total bytes written.
    #[inline]
    pub fn finish(mut self) -> usize {
        if self.bit_pos > 0 {
            self.buf[self.pos] = (self.bit_buf >> 56) as u8;
            self.pos += 1;
        }
        self.pos
    }

    /// Current byte position (bytes fully written so far).
    #[allow(dead_code)]
    #[inline]
    pub fn bytes_written(&self) -> usize {
        self.pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_byte_aligned() {
        let mut buf = [0u8; 4];
        let mut bw = BitWriter::new(&mut buf);
        bw.write(0xAB, 8);
        bw.write(0xCD, 8);
        let n = bw.finish();
        assert_eq!(n, 2);
        assert_eq!(&buf[..2], &[0xAB, 0xCD]);
    }

    #[test]
    fn test_write_unaligned() {
        let mut buf = [0u8; 4];
        let mut bw = BitWriter::new(&mut buf);
        bw.write(0b101, 3); // 3 bits: 101
        bw.write(0b11010, 5); // 5 bits: 11010
        let n = bw.finish();
        assert_eq!(n, 1);
        assert_eq!(buf[0], 0b10111010);
    }

    #[test]
    fn test_write_32bit() {
        let mut buf = [0u8; 8];
        let mut bw = BitWriter::new(&mut buf);
        bw.write(0xDEADBEEF, 32);
        let n = bw.finish();
        assert_eq!(n, 4);
        assert_eq!(&buf[..4], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }
}
