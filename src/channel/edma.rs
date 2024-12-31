//! Channel definition, implementation, for eDMA.

use crate::ral::{self, dmamux, tcd::BandwidthControl, Static};
use crate::{Error, SharedWaker};

use super::{Configuration, ConfigurationError};

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
    pub(crate) unsafe fn channel(&'static self, index: usize) -> Channel {
        assert!(index < CHANNELS);
        Channel {
            index,
            registers: match self.controller {
                crate::ral::Kind::EDma(registers) => registers,
            },
            multiplexer: self.multiplexer,
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
    registers: Static<crate::ral::dma::edma::RegisterBlock>,
    /// Reference to the DMA multiplexer
    multiplexer: Static<dmamux::RegisterBlock>,
    /// This channel's waker.
    pub(crate) waker: &'static SharedWaker,
}

impl Channel {
    pub(super) fn enable_impl(&self) {
        // Immutable write OK. No other methods directly modify ERQ.
        self.registers.SERQ.write(self.index as u8);
    }

    pub(super) fn tcd(&self) -> &crate::ral::tcd::RegisterBlock {
        &self.registers.TCD[self.index]
    }

    /// Set the channel's bandwidth control
    ///
    /// - `None` disables bandwidth control (default setting)
    /// - `Some(bwc)` sets the bandwidth control to `bwc`
    ///
    /// Note: This method is not available for eDMA3/eDMA4.
    pub fn set_bandwidth_control(&mut self, bandwidth: Option<BandwidthControl>) {
        let raw = BandwidthControl::raw(bandwidth);
        let tcd = self.tcd();
        crate::ral::modify_reg!(crate::ral::tcd, tcd, CSR, BWC: raw);
    }

    pub(super) fn set_channel_configuration_impl(
        &self,
        configuration: Configuration,
    ) -> Result<(), ConfigurationError> {
        // Immutable write OK. 32-bit store on configuration register.
        // eDMA3/4: Haven't found any equivalent to "always on." Doesn't seem
        // that the periodic request via PIT will apply, either.
        //
        // Hardware signals will route to the channel multiplexer configuration
        // register CHn_MUX in the TCD.
        let chcfg = &self.multiplexer.chcfg[self.index];
        match configuration {
            Configuration::Off => chcfg.write(0),
            Configuration::Enable { source, periodic } => {
                let mut v = source | dmamux::RegisterBlock::ENBL;
                if periodic {
                    if self.channel() < 4 {
                        return Err(ConfigurationError::PeriodicUnsupported);
                    }
                    v |= dmamux::RegisterBlock::TRIG;
                }
                chcfg.write(v);
                if chcfg.read() != v {
                    return Err(ConfigurationError::SourceCleared);
                }
            }
            Configuration::AlwaysOn => {
                // See note in reference manual: when A_ON is high, SOURCE is ignored.
                chcfg.write(dmamux::RegisterBlock::ENBL | dmamux::RegisterBlock::A_ON)
            }
        };
        Ok(())
    }

    pub(super) fn is_hardware_signaling_impl(&self) -> bool {
        self.registers.HRS.read() & (1 << self.index) != 0
    }

    pub(super) fn disable_impl(&self) {
        // Immutable write OK. No other methods directly modify ERQ.
        self.registers.CERQ.write(self.index as u8);
    }

    pub(super) fn is_interrupt_impl(&self) -> bool {
        self.registers.INT.read() & (1 << self.index) != 0
    }

    pub(super) fn clear_interrupt_impl(&self) {
        // Immutable write OK. No other methods modify INT.
        self.registers.CINT.write(self.index as u8);
    }

    pub(super) fn is_complete_impl(&self) -> bool {
        let tcd = self.tcd();
        crate::ral::read_reg!(crate::ral::tcd, tcd, CSR, DONE == 1)
    }

    pub(super) fn clear_complete_impl(&self) {
        // Immutable write OK. CDNE affects a bit in TCD. But, other writes to
        // TCD require &mut reference. Existence of &mut reference blocks
        // clear_complete calls.
        self.registers.CDNE.write(self.index as u8);
    }

    pub(super) fn is_error_impl(&self) -> bool {
        self.registers.ERR.read() & (1 << self.index) != 0
    }

    pub(super) fn clear_error_impl(&self) {
        // Immutable write OK. CERR affects a bit in ERR, which is
        // not written to elsewhere.
        self.registers.CERR.write(self.index as u8);
    }

    pub(super) fn is_active_impl(&self) -> bool {
        let tcd = self.tcd();
        ral::read_reg!(crate::ral::tcd, tcd, CSR, ACTIVE == 1)
    }

    pub(super) fn is_enabled_impl(&self) -> bool {
        self.registers.ERQ.read() & (1 << self.index) != 0
    }

    pub(super) fn error_status_impl(&self) -> Error {
        Error::new(self.registers.ES.read())
    }

    /// Returns `0` as the controller implementation.
    ///
    /// Even though 1160 and 1170 MCUs have two DMA controllers, eDMA0
    /// and eDMA1 (CM7 and CM4 LPSR domains, respectively), each DMA
    /// controller can reach all peripherals. See chapter 4.4 of the
    /// 1170 RM for details.
    ///
    /// It would be formally correct to return a `1` here for the eDMA1
    /// instance, but the DMA construction API in this package doesn't let
    /// users differentiate between eDMA0 and eDMA1. We could change this
    /// later without affecting correctness.
    pub(super) const fn controller_impl(&self) -> u8 {
        0
    }
}
