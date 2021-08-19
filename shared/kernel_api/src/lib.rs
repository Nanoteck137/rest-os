//! A Library to hold common kernel api components

#![no_std]
#![allow(dead_code)]

use core::convert::TryFrom;

#[repr(u64)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum KernelError {
    NoError = 0,
    TestError = 123,
}

impl TryFrom<u64> for KernelError {
    type Error = u64;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NoError),
            123 => Ok(Self::TestError),

            _ => Err(value),
        }
    }
}
