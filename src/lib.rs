#![cfg_attr(not(test), no_std)]
extern crate alloc;

//pub mod external_future;
mod timewrap;
pub use crate::timewrap::*;

#[cfg(feature = "drive_block")]
#[cfg(test)]
mod tests;
