//! Transfer Control Descriptor

#![allow(non_snake_case, non_upper_case_globals, clippy::module_inception)]

use super::RWRegister;

/// DMA Transfer Control Descriptor (TCD)
///
/// This layout works for all eDMA implementations, including
///
/// - Most 1000 and 1170 MCUs. (Referred to as just eDMA.)
/// - eDMA3 and eDMA4 on the 1180.
///
/// However, the *fields* may vary. See inline notes.
///
/// The TCD is technically larger in eDMA3 and eDMA4 IP blocks.
/// There's more registers at the start. For programming purposes,
/// they're otherwise identical.
#[repr(C, align(32))]
pub struct RegisterBlock {
    pub SADDR: RWRegister<u32>,
    // Signed numbers for offsets / 'last' members intentional.
    // The hardware treats them as signed numbers.
    pub SOFF: RWRegister<i16>,
    pub DATTR: RWRegister<u8>,
    pub SATTR: RWRegister<u8>,
    pub NBYTES: RWRegister<u32>,
    pub SLAST: RWRegister<i32>,
    pub DADDR: RWRegister<u32>,
    pub DOFF: RWRegister<i16>,
    /// The minor loop channel link field may vary in size
    /// depending on the implementation. Not worried right
    /// now, since we don't support minor loop linking.
    pub CITER: RWRegister<u16>,
    pub DLAST_SGA: RWRegister<i32>,
    /// These fields vary across all of eDMA, eDMA3 and
    /// eDMA4!
    ///
    /// Major loop channel linking field size changes as
    /// a function of the number of DMA channels.
    ///
    /// eDMA and eDMA3 have bandwidth control. eDMA4 has
    /// transfer mode control for read-only / write-only
    /// DMA transfers. Field is the same size.
    ///
    /// In the low byte, high nibble, eDMA has DONE and
    /// ACTIVE. eDMA3 and eDMA4 have things we probably don't
    /// need. Low byte, low nibble is the same.
    ///
    /// So we'll need to change how we handle DONE and
    /// ACTIVE access. They can't always dispatch to this
    /// register.
    pub CSR: RWRegister<u16>,
    /// See CITER documentation note about eDMA3 bitfield
    /// size when minor loop channel linking is enabled.
    pub BITER: RWRegister<u16>,
}

const _STATIC_ASSERT_TCD_32_BYTES: [u32; 1] =
    [0; (32 == core::mem::size_of::<RegisterBlock>()) as usize];

impl RegisterBlock {
    /// TCDs are uninitialized after reset. Set them to a known,
    /// good state here.
    pub fn reset(&self) {
        self.SADDR.write(0);
        self.SOFF.write(0);
        self.DATTR.write(0);
        self.SATTR.write(0);
        self.NBYTES.write(0);
        self.SLAST.write(0);
        self.DADDR.write(0);
        self.DOFF.write(0);
        self.CITER.write(0);
        self.DLAST_SGA.write(0);
        self.CSR.write(0);
        self.BITER.write(0);
    }
}

mod ATTR {
    /// Destination data transfer size
    pub mod SIZE {
        /// Offset (0 bits)
        pub const offset: u8 = 0;
        /// Mask (3 bits: 0b111 << 0)
        pub const mask: u8 = 0b111 << offset;
        /// Read-only values (empty)
        pub mod R {}
        /// Write-only values (empty)
        pub mod W {}
        /// Read-write values (empty)
        pub mod RW {}
    }

    /// Destination Address Modulo
    pub mod MOD {
        /// Offset (3 bits)
        pub const offset: u8 = 3;
        /// Mask (5 bits: 0b11111 << 3)
        pub const mask: u8 = 0b11111 << offset;
        /// Read-only values (empty)
        pub mod R {}
        /// Write-only values (empty)
        pub mod W {}
        /// Read-write values (empty)
        pub mod RW {}
    }
}

pub mod DATTR {
    pub use super::ATTR::*;
}

pub mod SATTR {
    pub use super::ATTR::*;
}

pub mod CSR {

    /// Enable an interrupt when major iteration count completes.
    pub mod INTMAJOR {
        /// Offset (1 bits)
        pub const offset: u16 = 1;
        /// Mask (1 bit: 1 << 1)
        pub const mask: u16 = 1 << offset;
        /// Read-only values (empty)
        pub mod R {}
        /// Write-only values (empty)
        pub mod W {}
        /// Read-write values
        pub mod RW {}
    }

    /// Disable Request
    pub mod DREQ {
        /// Offset (3 bits)
        pub const offset: u16 = 3;
        /// Mask (1 bit: 1 << 3)
        pub const mask: u16 = 1 << offset;
        /// Read-only values (empty)
        pub mod R {}
        /// Write-only values (empty)
        pub mod W {}
        /// Read-write values
        pub mod RW {}
    }

    /// Channel Done
    ///
    /// Only available for eDMA!
    pub mod DONE {
        /// Offset (7 bits)
        pub const offset: u16 = 7;
        /// Mask (1 bit: 1 << 7)
        pub const mask: u16 = 1 << offset;
        /// Read-only values (empty)
        pub mod R {}
        /// Write-only values (empty)
        pub mod W {}
        /// Read-write values (empty)
        pub mod RW {}
    }

    /// Bandwidth Control
    ///
    /// Only available for eDMA and eDMA3!
    pub mod BWC {
        /// Offset (14 bits)
        pub const offset: u16 = 14;
        /// Mask (2 bits: 0b11 << 14)
        pub const mask: u16 = 0b11 << offset;
        /// Read-only values (empty)
        pub mod R {}
        /// Write-only values (empty)
        pub mod W {}
        /// Read-write values
        pub mod RW {

            /// 0b00: No eDMA engine stalls.
            pub const BWC_0: u16 = 0b00;

            /// 0b10: eDMA engine stalls for 4 cycles after each R/W.
            pub const BWC_2: u16 = 0b10;

            /// 0b11: eDMA engine stalls for 8 cycles after each R/W.
            pub const BWC_3: u16 = 0b11;
        }
    }

    /// Channel Active
    ///
    /// Only available for eDMA!
    pub mod ACTIVE {
        /// Offset (6 bits)
        pub const offset: u16 = 6;
        /// Mask (1 bit: 1 << 6)
        pub const mask: u16 = 1 << offset;
        /// Read-only values (empty)
        pub mod R {}
        /// Write-only values (empty)
        pub mod W {}
        /// Read-write values (empty)
        pub mod RW {}
    }

    pub mod START {
        pub const offset: u16 = 0;
        pub const mask: u16 = 1 << offset;
        pub mod R {}
        pub mod W {}
        pub mod RW {}
    }
}

pub mod CITER {
    /// Current Major Iteration Count
    pub mod CITER {
        pub const offset: u16 = 0;
        pub const mask: u16 = 0x7fff << offset;
        pub mod R {}
        pub mod W {}
        pub mod RW {}
    }
}

pub mod BITER {
    /// Starting Major Iteration Count
    pub mod BITER {
        pub const offset: u16 = 0;
        pub const mask: u16 = 0x7fff << offset;
        pub mod R {}
        pub mod W {}
        pub mod RW {}
    }
}

/// Throttles the amount of bus bandwidth consumed by the eDMA
///
/// Defines the number of stalls that the DMA engine will insert
/// between most element transfers.
///
/// Some stalls may not occur to minimize startup latency. See the
/// reference manual for more details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum BandwidthControl {
    /// DMA engine stalls for 4 cycles after each R/W.
    Stall4Cycles = CSR::BWC::RW::BWC_2,
    /// DMA engine stalls for 8 cycles after each R/W.
    Stall8Cycles = CSR::BWC::RW::BWC_3,
}

impl BandwidthControl {
    pub(crate) fn raw(bwc: Option<Self>) -> u16 {
        match bwc {
            None => CSR::BWC::RW::BWC_0,
            Some(bwc) => bwc as u16,
        }
    }
}

/// TCD implementation for an eDMA IP block.
pub(crate) mod edma {
    pub use super::RegisterBlock;
}

/// TCD implementation for eDMA3 and eDMA4 blocks.
pub(crate) mod edma34 {
    use super::RWRegister;

    #[repr(C, align(32))]
    pub(crate) struct RegisterBlock {
        pub CSR: RWRegister<u32>,
        pub ES: RWRegister<u32>,
        pub INT: RWRegister<u32>,
        pub SBR: RWRegister<u32>,
        pub PRI: RWRegister<u32>,
        pub MUX: RWRegister<u32>,
        /// Only available on eDMA4, and reserved
        /// on eDMA3. We don't need it right now.
        _mattr: u32,
        pub TCD: super::RegisterBlock,
    }

    const _: () = assert!(core::mem::offset_of!(RegisterBlock, TCD) == 0x20);

    pub mod CSR {
        pub mod ACTIVE {
            pub const offset: u32 = 31;
            pub const mask: u32 = 1 << offset;
            pub mod R {}
            pub mod W {}
            pub mod RW {}
        }

        pub mod DONE {
            pub const offset: u32 = 30;
            pub const mask: u32 = 1 << offset;
            pub mod R {}
            pub mod W {}
            pub mod RW {}
        }

        pub mod ERQ {
            pub const offset: u32 = 0;
            pub const mask: u32 = 1 << offset;
            pub mod R {}
            pub mod W {}
            pub mod RW {}
        }
    }
}
