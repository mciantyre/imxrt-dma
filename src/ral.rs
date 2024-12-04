//! A RAL-like module to support DMA register access
//!
//! The RAL has TONS of symbols for DMA. The script that auto-generates
//! the RAL from a SVD file doesn't represent register clusters as an array
//! of structs. The transfer control descriptions, in particularly, could
//! conveniently be represented by 32 TCD structs. Same with the multiplexer
//! registers. Same with the priority registers...
//!
//! This module lets us hit those ideals. At the same time, we can expose an
//! interface that lets us use the RAL macros, where applicable.

#![allow(
    non_snake_case, // Compatibility with RAL
    unused, // Prototyping convenience
)]

pub mod dma;
pub mod dmamux;
pub mod tcd;

pub use ral_registers::{modify_reg, read_reg, write_reg};
use ral_registers::{RORegister, RWRegister, WORegister};

//
// Helper types for static memory
//
// Similar to the RAL's `Instance` type, but more copy.
//

pub(super) struct Static<T>(pub(super) *const T);
impl<T> core::ops::Deref for Static<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        // Safety: pointer points to static memory (peripheral memory)
        unsafe { &*self.0 }
    }
}
impl<T> Clone for Static<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Static<T> {}

/// Manages the kind of eDMA peripheral we're using.
///
/// I'd hope that the compiler can remove any runtime
/// dispatch when there's only one variant. But I'm
/// writing this without measuring that claim.
///
/// We'll likely need runtime dispatch for 1180 eDMA3
/// and eDMA4 selection (unless we adopt some kind of
/// type state). Let's make that the default repr
/// of our problem.
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    #[cfg(not(feature = "edma34"))]
    EDma(Static<dma::edma::RegisterBlock>),
    #[cfg(feature = "edma34")]
    EDma3(Static<dma::edma3::RegisterBlock>),
    #[cfg(feature = "edma34")]
    EDma4(Static<dma::edma4::RegisterBlock>),
}

impl Kind {
    /// Access the common TCD representation.
    ///
    /// You're reponsible for knowing if accesses on the TCD are
    /// correct for the implementation.
    pub(crate) fn tcd(&self, index: usize) -> &tcd::RegisterBlock {
        match self {
            #[cfg(not(feature = "edma34"))]
            Self::EDma(edma) => &edma.TCD[index],
            #[cfg(feature = "edma34")]
            Self::EDma3(edma3) => &edma3.TCD[index].TCD,
            #[cfg(feature = "edma34")]
            Self::EDma4(edma4) => &edma4.TCD[index].TCD,
        }
    }

    #[cfg(feature = "edma34")]
    pub(crate) fn channel(&self, index: usize) -> &tcd::edma34::RegisterBlock {
        match self {
            Self::EDma3(edma3) => &edma3.TCD[index],
            Self::EDma4(edma4) => &edma4.TCD[index],
        }
    }
    #[cfg(feature = "edma34")]
    pub(crate) fn is_hardware_signaling(&self, index: usize) -> bool {
        match self {
            Self::EDma3(edma3) => edma3.HRS.read() & 1 << index != 0,
            Self::EDma4(edma4) if index < 32 => edma4.HRS_LOW.read() & 1 << index != 0,
            Self::EDma4(edma4) if (32..64).contains(&index) => {
                edma4.HRS_HIGH.read() & 1 << (index - 32) != 0
            }
            _ => unreachable!("Driver guarantees that index is always in bounds"),
        }
    }
}

#[cfg(not(feature = "edma34"))]
impl core::ops::Deref for Kind {
    type Target = dma::edma::RegisterBlock;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::EDma(edma) => edma,
        }
    }
}
