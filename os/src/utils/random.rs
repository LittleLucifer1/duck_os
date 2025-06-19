use alloc::vec::Vec;
use alloc::vec;
use lazy_static::lazy_static;

use crate::sync::SpinNoIrqLock;

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: usize = 0x9908b0df;
const UPPER_MASK: usize = 0x8000_0000;
const LOWER_MASK: usize = 0x7fff_ffff;

// Algorithm: Mersenne twister
pub struct RandomGenerator {
    pub mt: Vec<usize>,
    pub mti: usize,
}

impl RandomGenerator {
    pub fn new() -> Self {
        RandomGenerator {
            mt: vec![0; N],
            mti: N + 1,
        }
    }

    pub fn init_genrand(&mut self, s: usize) {
        self.mt[0] = s & 0xffff_ffff;
        for mti in 1..N {
            self.mt[mti] = 1812433253 * (self.mt[mti-1] ^ (self.mt[mti-1] >> 30)) + mti;
            self.mt[mti] &= 0xffff_ffff;
        }
    }

    // Attention: 虽然返回值是usize，8个字节，但是函数实际上返回的是4字节有效的随机数
    pub fn genrand_u32(&mut self) -> usize {
        let mut y: usize;
        let mag01: Vec<usize> = vec![0x0, MATRIX_A];
        if self.mti >= N {
            if self.mti == N + 1 {
                self.init_genrand(5489);
            }
            for kk in 0..N-M {
                y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk+1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M] ^ (y >> 1) ^ mag01[y & 0x1usize];
            }
            for kk in N-M..N-1 {
                y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk+1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M - N] ^ (y >> 1) ^ mag01[y & 0x1usize];
            }
            y = (self.mt[N-1] & UPPER_MASK) | (self.mt[0] & LOWER_MASK);
            self.mt[N-1] = self.mt[M-1] ^ (y >> 1) ^ mag01[y & 0x1usize];
            self.mti = 0;
        }
        self.mti += 1;
        y = self.mt[self.mti];
        y ^= y >> 1;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    pub fn write_to_buf(&mut self, buf: &mut [u8]) {
        let mut offset = 0;
    
        while offset < buf.len() {
            let num = self.genrand_u32().to_be_bytes();
            let bytes_to_copy = (buf.len() - offset).min(4);
            buf[offset..offset + bytes_to_copy].copy_from_slice(&num[..bytes_to_copy]);
            offset += bytes_to_copy;
        }
    }
    
}

lazy_static! {
    pub static ref RANDOM_GENERATOR: SpinNoIrqLock<RandomGenerator> = SpinNoIrqLock::new(RandomGenerator::new());
}