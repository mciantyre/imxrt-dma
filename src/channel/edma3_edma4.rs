//! Channel definitions, implementation, for eDMA3 / eDMA4.

use crate::ral::{self, Kind};
use crate::{Error, SharedWaker};

use super::Configuration;

impl<const CHANNELS: usize> crate::Dma<CHANNELS> {
    /// Creates the DMA channel described by `index`.
    ///
    /// # Safety
    ///
    /// This will create a handle that may alias global, mutable state. You should only create
    /// one channel per index. If there are multiple channels for the same index, you're
    /// responsible for ensuring synchronized access.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than or equal to the maximum number of channels.
    pub unsafe fn channel(&'static self, index: usize) -> Channel {
        assert!(index < CHANNELS);
        Channel {
            index,
            registers: self.controller,
            waker: &self.wakers[index],
        }
    }
}

/// A DMA channel
///
/// You should rely on your HAL to allocate `Channel`s. If your HAL does not allocate channels,
/// or if you're desigining the HAL, use [`Dma`](crate::Dma) to create channels.
///
/// The `Channel` stores memory addresses independent of the memory lifetime. You must make
/// sure that the channel's state is valid before enabling a transfer!
pub struct Channel {
    /// Our channel number, expected to be between [0, 32)
    pub(super) index: usize,
    /// Reference to the DMA registers
    registers: Kind,
    /// This channel's waker.
    pub(crate) waker: &'static SharedWaker,
}

impl Channel {
    pub(super) fn enable_impl(&self) {
        // eDMA3/4: dispatch to the TCD CHn_CSR. RMW on bit
        // 0 to enable. Immutable write still OK: channel
        // deemed unique, and it should be !Sync.
        let chan = self.channel_registers();
        ral::modify_reg!(crate::ral::tcd::edma34, chan, CSR, ERQ: 1);
    }

    fn channel_registers(&self) -> &ral::tcd::edma34::RegisterBlock {
        match &self.registers {
            Kind::EDma3(edma3) => &edma3.TCD[self.index],
            Kind::EDma4(edma4) => &edma4.TCD[self.index],
        }
    }

    /// Returns a handle to this channel's transfer control descriptor.
    pub(super) fn tcd(&self) -> &crate::ral::tcd::RegisterBlock {
        match &self.registers {
            Kind::EDma3(edma3) => &edma3.TCD[self.index].TCD,
            Kind::EDma4(edma4) => &edma4.TCD[self.index].TCD,
        }
    }

    pub(super) fn set_channel_configuration_impl(&mut self, configuration: Configuration) {
        let source = match configuration {
            Configuration::Off => 0,
            Configuration::Enable { source } => source,
        };
        let chan = self.channel_registers();
        ral::write_reg!(crate::ral::tcd::edma34, chan, MUX, source);
    }

    pub(super) fn is_hardware_signaling_impl(&self) -> bool {
        match &self.registers {
            Kind::EDma3(edma3) => edma3.HRS.read() & 1 << self.index != 0,
            Kind::EDma4(edma4) if self.index < 32 => edma4.HRS_LOW.read() & 1 << self.index != 0,
            Kind::EDma4(edma4) if (32..64).contains(&self.index) => {
                edma4.HRS_HIGH.read() & 1 << (self.index - 32) != 0
            }
            _ => unreachable!("Driver guarantees that index is always in bounds"),
        }
    }

    pub(super) fn disable_impl(&self) {
        // eDMA3/4: see notes in enable. RMW to set bit 0 low.
        let chan = self.channel_registers();
        ral::modify_reg!(crate::ral::tcd::edma34, chan, CSR, ERQ: 0);
    }

    pub(super) fn is_interrupt_impl(&self) -> bool {
        // eDMA3/4: Each channel has a W1C interrupt bit.
        // Prefer that instead of the aggregate register(s)
        // in the MP space.
        self.channel_registers().INT.read() != 0
    }

    pub(super) fn clear_interrupt_impl(&self) {
        // eDMA3/4: See note in is_interrupt.
        self.channel_registers().INT.write(1);
    }

    pub(super) fn is_complete_impl(&self) -> bool {
        // eDMA3/4: Need to check CHn_CSR in the TCD space.
        let chan = self.channel_registers();
        ral::read_reg!(crate::ral::tcd::edma34, chan, CSR, DONE == 1)
    }

    pub(super) fn clear_complete_impl(&self) {
        // eDMA3/4: Need to change a CHn_CSR bit in the TCD space.
        let chan = self.channel_registers();
        ral::modify_reg!(crate::ral::tcd::edma34, chan, CSR, DONE: 1);
    }

    pub(super) fn is_error_impl(&self) -> bool {
        // eDMA3/4: Check CHn_ES, highest bit.
        self.channel_registers().ES.read() != 0
    }

    pub(super) fn clear_error_impl(&self) {
        // eDMA3/4: W1C CHn_ES, highest bit.
        self.channel_registers().ES.write(1 << 31);
    }

    pub(super) fn is_active_impl(&self) -> bool {
        // eDMA3/4: Check CHn_CSR, highest bit.
        let chan = self.channel_registers();
        ral::read_reg!(crate::ral::tcd::edma34, chan, CSR, ACTIVE == 1)
    }

    pub(super) fn is_enabled_impl(&self) -> bool {
        // eDMA3/4: Check CHn_CSR, lowest bit.
        let chan = self.channel_registers();
        ral::read_reg!(crate::ral::tcd::edma34, chan, CSR, ERQ == 1)
    }

    pub(super) fn error_status_impl(&self) -> Error {
        Error::new(self.channel_registers().ES.read())
    }
}
