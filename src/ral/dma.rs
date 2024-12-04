//! DMA register blocks and fields

use super::{tcd, RORegister, RWRegister, WORegister};

/// eDMA controller representation.
pub(crate) mod edma {
    use super::{tcd, RORegister, RWRegister, WORegister};

    use core::ops::Index;

    /// DMA registers.
    #[repr(C)]
    pub struct RegisterBlock {
        /// Control Register
        pub CR: RWRegister<u32>,
        /// Error Status Register
        pub ES: RORegister<u32>,
        _reserved1: [u32; 1],
        /// Enable Request Register
        pub ERQ: RWRegister<u32>,
        _reserved2: [u32; 1],
        /// Enable Error Interrupt Register
        pub EEI: RWRegister<u32>,
        /// Clear Enable Error Interrupt Register
        pub CEEI: WORegister<u8>,
        /// Set Enable Error Interrupt Register
        pub SEEI: WORegister<u8>,
        /// Clear Enable Request Register
        pub CERQ: WORegister<u8>,
        /// Set Enable Request Register
        pub SERQ: WORegister<u8>,
        /// Clear DONE Status Bit Register
        pub CDNE: WORegister<u8>,
        /// Set START Bit Register
        pub SSRT: WORegister<u8>,
        /// Clear Error Register
        pub CERR: WORegister<u8>,
        /// Clear Interrupt Request Register
        pub CINT: WORegister<u8>,
        _reserved3: [u32; 1],
        /// Interrupt Request Register
        pub INT: RWRegister<u32>,
        _reserved4: [u32; 1],
        /// Error Register
        pub ERR: RWRegister<u32>,
        _reserved5: [u32; 1],
        /// Hardware Request Status Register
        pub HRS: RORegister<u32>,
        _reserved6: [u32; 3],
        /// Enable Asynchronous Request in Stop Register
        pub EARS: RWRegister<u32>,
        _reserved7: [u32; 46],
        /// Channel Priority Registers
        pub DCHPRI: ChannelPriorityRegisters,
        _reserved8: [u32; 952],
        /// Transfer Control Descriptors
        pub TCD: [tcd::RegisterBlock; 32],
    }

    /// Wrapper for channel priority registers
    ///
    /// Channel priority registers cannot be accessed with
    /// normal channel indexes. This adapter makes it so that
    /// we *can* access them with channel indexes by converting
    /// the channel number to a reference to the priority
    /// register.
    #[repr(transparent)]
    pub struct ChannelPriorityRegisters([RWRegister<u8>; 32]);

    impl Index<usize> for ChannelPriorityRegisters {
        type Output = RWRegister<u8>;
        fn index(&self, channel: usize) -> &RWRegister<u8> {
            // Pattern follows
            //
            //   3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, ...
            //
            // for all channels. NXP keeping us on our toes. They're
            // really keeping us on our toes, because this only applies
            // to eDMA.
            let idx = 4 * (channel / 4) + (3 - (channel % 4));
            &self.0[idx]
        }
    }
}

/// eDMA3 controller representation.
pub(crate) mod edma3 {
    use super::{tcd, RORegister, RWRegister};

    #[repr(C)]
    pub struct RegisterBlock {
        pub CSR: RWRegister<u32>,
        pub ES: RORegister<u32>,
        pub INT: RORegister<u32>,
        pub HRS: RORegister<u32>,
        _reserved0: [u8; 0x100 - 0x10],
        pub GRPRI: [u32; 32],
        _reserved1: [u8; 0x1_0000 - 0x180],
        pub TCD: [tcd::edma34::RegisterBlock; 32],
    }

    // Did I calculate my reservations correctly?
    const _: () = assert!(core::mem::offset_of!(RegisterBlock, GRPRI) == 0x100);

    // DMA3.TCD base address: 4401_0000h
    // DMA3.MP base address: 4400_0000h.
    //
    // That means the difference is...
    const _: () = assert!(core::mem::offset_of!(RegisterBlock, TCD) == 0x1_0000);
}

/// eDMA4 controller representation.
///
/// Nearly the same as eDMA3, but there's extra registers to account
/// for the extra DMA channels. Too bad they couldn't reserve some
/// registers. (There's other tricks we could use here, if we moved)
pub(crate) mod edma4 {
    use super::{tcd, RORegister, RWRegister};

    #[repr(C)]
    pub struct RegisterBlock {
        pub CSR: RWRegister<u32>,
        pub ES: RORegister<u32>,
        pub INT_LOW: RORegister<u32>,
        pub INT_HIGH: RORegister<u32>,
        pub HRS_LOW: RORegister<u32>,
        pub HRS_HIGH: RORegister<u32>,
        _reserved0: [u8; 0x100 - 0x18],
        pub GRPRI: [u32; 64],
        _reserved1: [u8; 0x1_0000 - 0x200],
        pub TCD: [tcd::edma34::RegisterBlock; 32],
    }

    // Did I calculate my reservations correctly?
    const _: () = assert!(core::mem::offset_of!(RegisterBlock, GRPRI) == 0x100);

    // DMA4.MP base address: 4200_0000h
    // DMA4.TCD base address: 4201_0000h
    //
    // Assuming the user provides the proper eDMA4 pointer, that means the
    // difference is...
    const _: () = assert!(core::mem::offset_of!(RegisterBlock, TCD) == 0x1_0000);
}
