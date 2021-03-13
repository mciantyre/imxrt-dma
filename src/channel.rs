//! DMA channel

use core::mem;

use crate::{
    element::Element,
    ral::{self, dma, dmamux, tcd::BandwidthControl, Static, DMA, MULTIPLEXER},
    ErrorStatus,
};

/// A DMA channel
///
/// You should rely on your HAL to allocate `Channel`s. If your HAL does not allocate channels,
/// or if you're desigining the HAL, use [`new`](#method.new) to create a new DMA channel.
///
/// You must always specify the source and destination transfer descriptors before enabling the
/// transfer.
pub struct Channel {
    /// Our channel number, expected to be between 0 to (CHANNEL_COUNT - 1)
    index: usize,
    /// Reference to the DMA registers
    registers: Static<dma::RegisterBlock>,
    /// Reference to the DMA multiplexer
    multiplexer: Static<dmamux::RegisterBlock>,
}

impl Channel {
    /// Set the channel's bandwidth control
    ///
    /// - `None` disables bandwidth control (default setting)
    /// - `Some(bwc)` sets the bandwidth control to `bwc`
    pub fn set_bandwidth_control(&mut self, bandwidth: Option<BandwidthControl>) {
        let raw = BandwidthControl::raw(bandwidth);
        let tcd = self.tcd();
        ral::modify_reg!(crate::ral::tcd, tcd, CSR, BWC: raw);
    }

    /// Returns the DMA channel number
    ///
    /// Channels are unique and numbered within the half-open range `[0, CHANNEL_COUNT)`.
    pub fn channel(&self) -> usize {
        self.index
    }

    /// Creates the DMA channel described by `index`
    ///
    /// # Safety
    ///
    /// This will create a handle that may alias global, mutable state.
    ///
    /// You must make sure that `index` describes a valid DMA channel for your system.
    /// If you're using this driver on a i.MX RT 1010 processor, you must make sure
    /// that `index` is less than 16.
    ///
    /// # Panics
    ///
    /// Panics if `index` is greater than 32.
    #[inline(always)]
    pub unsafe fn new(index: usize) -> Self {
        // TODO consider breaking the API and return `Option<Channel>`
        if index < 32 {
            Channel {
                index,
                registers: DMA,
                multiplexer: MULTIPLEXER,
            }
        } else {
            panic!("DMA channel index {} exceeds CHANNEL_COUNT", index);
        }
    }

    /// Reset the transfer control descriptor owned by the DMA channel
    ///
    /// `reset` should be called during channel initialization to put the
    /// channel into a known, good state.
    pub fn reset(&mut self) {
        self.tcd().reset();
    }

    /// Returns a handle to this channel's transfer control descriptor
    fn tcd(&self) -> &crate::ral::tcd::RegisterBlock {
        &self.registers.TCD[self.index]
    }

    /// Prepare the source of a transfer; see [`Transfer`](struct.Transfer.html) for details.
    ///
    /// # Safety
    ///
    /// User must ensure that the memory described by `Transfer` is valid for the lifetime of
    /// the DMA transaction.
    pub unsafe fn set_source_transfer<E: Element>(&mut self, transfer: &Transfer<E>) {
        let tcd = self.tcd();
        ral::write_reg!(crate::ral::tcd, tcd, SADDR, transfer.address as u32);
        ral::write_reg!(crate::ral::tcd, tcd, SOFF, transfer.offset);
        ral::modify_reg!(crate::ral::tcd, tcd, ATTR, SSIZE: E::DATA_TRANSFER_ID, SMOD: transfer.modulo);
        ral::write_reg!(
            crate::ral::tcd,
            tcd,
            SLAST,
            transfer.last_address_adjustment
        );
    }

    /// Prepare the destination for a transfer; see [`Transfer`](struct.Transfer.html) for details.
    ///
    /// # Safety
    ///
    /// User must ensure that the memory described by `Transfer` is valid for the lifetime of
    /// the DMA transaction.
    pub unsafe fn set_destination_transfer<E: Element>(&mut self, transfer: &Transfer<E>) {
        let tcd = self.tcd();
        ral::write_reg!(crate::ral::tcd, tcd, DADDR, transfer.address as u32);
        ral::write_reg!(crate::ral::tcd, tcd, DOFF, transfer.offset);
        ral::modify_reg!(crate::ral::tcd, tcd, ATTR, DSIZE: E::DATA_TRANSFER_ID, DMOD: transfer.modulo);
        ral::write_reg!(
            crate::ral::tcd,
            tcd,
            DLAST_SGA,
            transfer.last_address_adjustment
        );
    }

    /// Set the number of *bytes* to transfer per minor loop
    ///
    /// Describes how many bytes we should transfer for each DMA service request.
    pub fn set_minor_loop_bytes(&self, nbytes: u32) {
        // Immutable write OK. 32-bit store on NBYTES.
        let tcd = self.tcd();
        ral::write_reg!(crate::ral::tcd, tcd, NBYTES, nbytes);
    }

    /// Se the number of elements to move in each minor loop
    ///
    /// Describes how many elements we should transfer for each DMA service request.
    pub fn set_minor_loop_elements<E: Element>(&self, len: usize) {
        // Immutable write OK. See set_minor_loop_bytes.
        self.set_minor_loop_bytes((mem::size_of::<E>() * len) as u32);
    }

    /// Tells the DMA channel how many transfer iterations to perform
    ///
    /// A 'transfer iteration' is a read from a source, and a write to a destination, with
    /// read and write sizes described by a minor loop. Each iteration requires a DMA
    /// service request, either from hardware or from software.
    pub fn set_transfer_iterations(&mut self, iterations: u16) {
        let tcd = self.tcd();
        ral::write_reg!(crate::ral::tcd, tcd, CITER, iterations);
        ral::write_reg!(crate::ral::tcd, tcd, BITER, iterations);
    }

    /// Set the DMAMUX channel configuration
    ///
    /// See the [`ChannelConfiguration`](crate::channel::ChannelConfiguration) documentation
    /// for more information.
    ///
    /// # Panics
    ///
    /// Only the first four DMA channels support periodic triggering from PIT timers. This method
    /// panics if `triggering` is set for the [`Enable`](crate::channel::ChannelConfiguration)
    /// variant, but the channel does not support triggering.
    pub fn set_channel_configuration(&self, configuration: ChannelConfiguration) {
        // Immutable write OK. 32-bit store on configuration register.
        let chcfg = &self.multiplexer.chcfg[self.index];
        match configuration {
            ChannelConfiguration::Off => chcfg.write(0),
            ChannelConfiguration::Enable { source, periodic } => {
                let mut v = source | dmamux::RegisterBlock::ENBL;
                if periodic {
                    assert!(
                        self.channel() < 4,
                        "Requested DMA periodic triggering on an unsupported channel."
                    );
                    v |= dmamux::RegisterBlock::TRIG;
                }
                chcfg.write(v);
            }
            ChannelConfiguration::AlwaysOn => {
                // See note in reference manual: when A_ON is high, SOURCE is ignored.
                chcfg.write(dmamux::RegisterBlock::ENBL | dmamux::RegisterBlock::A_ON)
            }
        }
    }

    /// Returns `true` if the DMA channel is receiving a service signal from hardware
    pub fn is_hardware_signaling(&self) -> bool {
        self.registers.HRS.read() & (1 << self.index) != 0
    }

    /// Enable the DMA multiplexer request, which signals that the transfer is
    /// ready
    ///
    /// # Safety
    ///
    /// This could initiate a DMA transaction that uses an invalid source or destination.
    /// Caller must ensure that the source and destination transfer descriptors are valid.
    /// See [`set_source_transfer`](#method.set_source_transfer) and
    /// [`set_destination_transfer`](#method.set_destination_transfer) for more information.
    pub unsafe fn enable(&self) {
        // Immutable write OK. No other methods directly modify ERQ.
        self.registers.SERQ.write(self.index as u8);
    }

    /// Disable the DMA channel, preventing any DMA transfers
    pub fn disable(&self) {
        // Immutable write OK. No other methods directly modify ERQ.
        self.registers.CERQ.write(self.index as u8);
    }

    /// Returns `true` if this DMA channel generated an interrupt
    pub fn is_interrupt(&self) -> bool {
        self.registers.INT.read() & (1 << self.index) != 0
    }

    /// Clear the interrupt flag from this DMA channel
    pub fn clear_interrupt(&self) {
        // Immutable write OK. No other methods modify INT.
        self.registers.CINT.write(self.index as u8);
    }

    /// Enable or disable 'disable on completion'
    ///
    /// 'Disable on completion' lets the DMA channel automatically clear the request signal
    /// when it completes a transfer.
    pub fn set_disable_on_completion(&mut self, dreq: bool) {
        let tcd = self.tcd();
        ral::modify_reg!(crate::ral::tcd, tcd, CSR, DREQ: dreq as u16);
    }

    /// Enable or disable interrupt generation when the transfer completes
    ///
    /// You're responsible for registering your interrupt handler.
    pub fn set_interrupt_on_completion(&mut self, intr: bool) {
        let tcd = self.tcd();
        ral::modify_reg!(crate::ral::tcd, tcd, CSR, INTMAJOR: intr as u16);
    }

    /// Indicates if the DMA transfer has completed
    pub fn is_complete(&self) -> bool {
        let tcd = self.tcd();
        ral::read_reg!(crate::ral::tcd, tcd, CSR, DONE == 1)
    }

    /// Clears completion indication
    pub fn clear_complete(&self) {
        // Immutable write OK. CDNE affects a bit in TCD. But, other writes to
        // TCD require &mut reference. Existence of &mut reference blocks
        // clear_complete calls.
        self.registers.CDNE.write(self.index as u8);
    }

    /// Indicates if the DMA channel is in an error state
    pub fn is_error(&self) -> bool {
        self.registers.ERR.read() & (1 << self.index) != 0
    }

    /// Clears the error flag
    pub fn clear_error(&self) {
        // Immutable write OK. CERR affects a bit in ERR, which is
        // not written to elsewhere.
        self.registers.CERR.write(self.index as u8);
    }

    /// Indicates if this DMA channel is actively transferring data
    pub fn is_active(&self) -> bool {
        let tcd = self.tcd();
        ral::read_reg!(crate::ral::tcd, tcd, CSR, ACTIVE == 1)
    }

    /// Indicates if this DMA channel is enabled
    pub fn is_enabled(&self) -> bool {
        self.registers.ERQ.read() & (1 << self.index) != 0
    }

    /// Returns the value from the **global** error status register
    ///
    /// It may reflect the last channel that produced an error, and that
    /// may not be related to this channel.
    pub fn error_status(&self) -> ErrorStatus {
        ErrorStatus::new(self.registers.ES.read())
    }

    /// Start a DMA transfer
    ///
    /// `start()` should be used to request service from the DMA controller. It's
    /// necessary for in-memory DMA transfers. Do not use it for hardware-initiated
    /// DMA transfers. DMA transfers that involve hardware will rely on the hardware
    /// to request DMA service.
    ///
    /// Flag is automatically cleared by hardware after it's asserted.
    ///
    /// # Safety
    ///
    /// This could initiate a DMA transaction that uses an invalid source or destination.
    /// Caller must ensure that the source and destination transfer descriptors are valid.
    /// See [`set_source_transfer`](#method.set_source_transfer) and
    /// [`set_destination_transfer`](#method.set_destination_transfer) for more information.
    pub unsafe fn start(&self) {
        // Immutable write OK. SSRT affects a bit in TCD. But, other writes to
        // TCD require &mut reference. Existence of &mut reference blocks
        // start calls.
        self.registers.SSRT.write(self.index as u8);
    }
}

/// Describes a DMA transfer
///
/// `Transfer` describes a source or a destination of a DMA transfer. A source or destination
/// could be
///
/// - a hardware register
/// - an element buffer that's treated as linear memory
/// - an element buffer that's treated as a circular buffer
///
/// A transfer that uses a circular buffer requires that the buffer size is a power of two.
///
/// It's always safe to create a `Transfer`, because the struct is inert. But, it's generally
/// unsafe to use `Transfer` in other methods. You must make sure that the memory described by
/// `Transfer` is valid for the lifetime of the DMA transaction.
#[derive(Debug)]
pub struct Transfer<E: Element> {
    /// The starting address for the DMA transfer
    ///
    /// If this describes a source, `address` will be the first
    /// address read. If this describes a destination, `address`
    /// will be the first address written.
    address: *const E,

    /// Offsets to perform for each read / write of a memory address.
    ///
    /// When defining a transfer for a peripheral source or destination,
    /// `offset` should be zero. Otherwise, `offset` should represent the
    /// size of the data element, `E`.
    ///
    /// Negative (backwards) adjustments are permitted, if you'd like to read
    /// a buffer backwards or something.
    offset: i16,

    /* size: u16, // Not needed; captured in E: Element type */
    /// Defines the strategy for reading / writing linear or circular buffers
    ///
    /// `modulo` should be zero if this definition defines a transfer from linear
    /// memory or a peripheral. `modulo` will be non-zero when defining a transfer
    /// from a circular buffer. The non-zero value is the number of high bits to freeze
    /// when performing address offsets (see `offset`). Given that we're only supporting
    /// power-of-two buffer sizes, `modulo` will be `31 - clz(cap * sizeof(E))`, where `cap` is the
    /// total size of the circular buffer, `clz` is "count leading zeros," and `sizeof(E)` is
    /// the size of the element, in bytes.
    modulo: u16,

    /// Perform any last-address adjustments when we complete the transfer
    ///
    /// Once we complete moving data from a linear buffer, we should set our pointer back to the
    /// initial address. For this case, `last_address_adjustment` should be a negative number that
    /// describes how may *bytes* to move backwards from our current address to reach our starting
    /// address. Alternatively, it could describe how to move to a completely new address, like
    /// a nearby buffer that we're using for a double-buffer. Or, set it to zero, which means "keep
    /// your current position." "Keep your current position" is important when working with a
    /// peripheral address!
    last_address_adjustment: i32,
}

impl<E: Element> Transfer<E> {
    /// Defines a transfer that reads from a hardware register at `address`
    ///
    /// # Safety
    ///
    /// Caller must ensure that `address` is a memory location that can accept
    /// reads or writes from the DMA controller.
    pub unsafe fn hardware(address: *const E) -> Self {
        Transfer {
            address,
            // Don't move the address pointer
            offset: 0,
            // We're not a circular buffer
            modulo: 0,
            // Don't move the address pointer
            last_address_adjustment: 0,
        }
    }

    /// Defines a transfer that can read from or write to `buffer`
    ///
    /// `ptr` points to the starting element of the buffer. `len` indicates how many elements
    /// you will transfer
    ///
    /// # Safety
    ///
    /// Caller must ensure that the memory starting at `ptr` is valid for `len` elements.
    pub unsafe fn buffer_linear(ptr: *const E, len: usize) -> Self {
        // TODO drop `len`, and leave the last address adjustment as zero.
        // The implementation will always specifying the starting address,
        // so last address adjustment doesn't matter.
        Transfer {
            address: ptr,
            offset: core::mem::size_of::<E>() as i16,
            modulo: 0,
            last_address_adjustment: ((len * mem::size_of::<E>()) as i32).wrapping_neg(),
        }
    }

    /// Defines a transfer that can read from or write to the circular buffer
    ///
    /// `start` points to the first element that will be used in the transfer. `capacity`
    /// is the total size of the allocated memory region for the transfer; it is **not**
    /// the number of elements to transfer. `capacity` will be converted into the DMA
    /// transfer modulus value.
    ///
    /// # Safety
    ///
    /// `start` is a pointer somewhere in a linear buffer. The *alignment* of the that
    /// complete buffer must be a multiple of the buffer's size, in bytes. You must take
    /// care of buffer alignment.
    pub unsafe fn buffer_circular(start: *const E, capacity: usize) -> Option<Self> {
        if !capacity.is_power_of_two() {
            return None;
        }

        let modulo = 31 - (capacity * mem::size_of::<E>()).leading_zeros() as u16;
        Some(Transfer {
            address: start,
            offset: core::mem::size_of::<E>() as i16,
            modulo,
            last_address_adjustment: 0,
        })
    }
}

// It's OK to send a channel across an execution context.
// They can't be cloned or copied, so there's no chance of
// them being (mutably) shared.
unsafe impl Send for Channel {}

/// DMAMUX channel configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChannelConfiguration {
    /// The DMAMUX channel is disabled
    Off,
    /// The DMAMUX is enabled, permitting hardware triggering.
    /// See [`enable()`](ChannelConfiguration::enable) to enable
    /// the channel without periodic triggering.
    Enable {
        /// The DMA channel source (slot number)
        ///
        /// Specifies which DMA source is routed to the DMA channel.
        source: u32,
        /// Set the periodic triggering flag to schedule DMA transfers on PIT
        /// timer scheduling.
        ///
        /// `periodic` only works for the first four DMA channels, since
        /// it corresponds to the PIT timers.
        periodic: bool,
    },
    /// The DMAMUX is always on, and there's no need for software
    /// or hardware activation
    ///
    /// Use `AlwaysOn` for
    /// - memory-to-memory transfers
    /// - memory to external bus transfers
    AlwaysOn,
}

impl ChannelConfiguration {
    /// Enable the channel without triggering
    ///
    /// Shorthand for `ChannelConfiguration::Enable { source, periodic: false }`.
    /// Use `enable()` to avoid possible panics in
    /// [`set_channel_configuration`](crate::Channel::set_channel_configuration).
    pub const fn enable(source: u32) -> Self {
        ChannelConfiguration::Enable {
            source,
            periodic: false,
        }
    }
}
