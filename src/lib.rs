//! Direct Memory Access (DMA) driver for i.MX RT processors.
//!
//! `imxrt-dma` provides
//!
//! - an unsafe API for defining and scheduling transfers with DMA `Channel`s.
//! - safe DMA futures for memcpy, peripheral-to-memory, and memory-to-peripheral
//!   transfers.
//!
//! This DMA driver may be re-exported from a hardware abstraction layer
//! (HAL). If it is, you should use the safer APIs provided by your HAL.
//!
//! # Getting started
//!
//! To allocate a [`Dma`] driver, you'll need to know
//!
//! 1. the location of the DMA controller registers.
//! 2. the location of the DMAMUX registers.
//! 3. the number of DMA channels supported by your chip.
//!
//! These parameters depend on the i.MX RT chip you're targeting. If you're
//! already using [`imxrt-ral`](https://docs.rs/imxrt-ral), consider using the
//! `DMA` and `DMAMUX` constants for the addresses. You're always responsible
//! for configuring the number of DMA channels.
//!
//! With those three parameters, assign a `Dma` to a static. Then, use that
//! object to create DMA [`Channel`](crate::channel::Channel)s.
//!
//! ```
//! use imxrt_dma::Dma;
//! # const DMA_PTR: *const () = core::ptr::null() as _;
//! # const DMAMUX_PTR: *const () = core::ptr::null() as  _;
//!
//! // Safety: addresses and channel count are valid for this target.
//! static DMA: Dma<32> = unsafe { Dma::new(DMA_PTR, DMAMUX_PTR) };
//!
//! // Safety: we only allocate one DMA channel 7 object.
//! let mut channel = unsafe { DMA.channel(7) };
//! ```
//!
//! Once you have a channel, you can use the higher-level DMA APIs, like
//!
//! - [`memcpy`](crate::memcpy::memcpy) for memory copies.
//! - [`write`](crate::peripheral::write) to transmit data from memory to
//!   a peripheral.
//! - [`read`](crate::peripheral::read) to receive data from a peripheral.
//! - [`full_duplex`](crate::peripheral::full_duplex) to read / write with a
//!   peripheral using a single buffer.
//!
//! Peripheral transfers depends on a peripheral's DMA support. These are signaled
//! through various [`peripheral`] traits.
//!
//! For a lower-level API, use the [`channel`] objects and helper
//! functions.
//!
//! ### License
//!
//! Licensed under either of
//!
//! - [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0) ([LICENSE-APACHE](./LICENSE-APACHE))
//! - [MIT License](http://opensource.org/licenses/MIT) ([LICENSE-MIT](./LICENSE-MIT))
//!
//! at your option.
//!
//! Unless you explicitly state otherwise, any contribution intentionally submitted
//! for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
//! dual licensed as above, without any additional terms or conditions.

#![no_std]

pub mod channel;
mod element;
mod error;
mod interrupt;
pub mod memcpy;
pub mod peripheral;
mod ral;

pub use element::Element;
pub use error::Error;
pub use interrupt::Transfer;
pub use ral::tcd::BandwidthControl;

/// A DMA result
pub type Result<T> = core::result::Result<T, Error>;

/// A DMA driver.
///
/// This DMA driver manages the DMA controller and the multiplexer.
/// It's configured with pointers to both peripherals.
///
/// `Dma` allocates [`Channel`](channel::Channel)s. `Channel` provides
/// the interface for scheduling transfers.
///
/// `DMA_INST` denotes the eDMA controller instance that you're using. For
/// example, if you're using eDMA3, `DMA_INST` is `3`. If your eDMA instance
/// doesn't have a number, set `DMA_INST` to `0`.
pub struct Dma<const DMA_INST: u8, const CHANNELS: usize> {
    controller: ral::Kind,
    #[cfg(not(feature = "edma34"))]
    multiplexer: ral::Static<ral::dmamux::RegisterBlock>,
    wakers: [SharedWaker; CHANNELS],
}

// Safety: OK to allocate a DMA driver in a static context.
unsafe impl<const DMA_INST: u8, const CHANNELS: usize> Sync for Dma<DMA_INST, CHANNELS> {}

#[cfg(not(feature = "edma34"))]
impl<const CHANNELS: usize> Dma<0, CHANNELS> {
    /// Create the DMA driver.
    ///
    /// Note that this can evaluate at compile time. Consider using this to
    /// expose a `Dma` through your higher-level API that you can use to
    /// allocate DMA channels.
    ///
    /// `CHANNELS` specifies the total number of channels supported by the DMA
    /// controller. It's referenced when allocating channels.
    ///
    /// # Safety
    ///
    /// Caller must make sure that `controller` is a pointer to the start of the
    /// DMA controller register block. Caller must also make sure that
    /// `multiplexer` is a pointer to the start of the DMA multiplexer. Both
    /// pointers must be valid for your MCU.
    ///
    /// An incorrect `CHANNELS` value prevents proper bounds checking when
    /// allocating channels. This may result in DMA channels that point to
    /// invalid memory.
    pub const unsafe fn new(controller: *const (), multiplexer: *const ()) -> Self {
        Self {
            controller: ral::Kind::EDma(ral::Static(controller.cast())),
            multiplexer: ral::Static(multiplexer.cast()),
            wakers: [NO_WAKER; CHANNELS],
        }
    }
}

#[cfg(feature = "edma34")]
impl<const CHANNELS: usize> Dma<3, CHANNELS> {
    /// Create an eDMA3 driver.
    ///
    /// # Safety
    ///
    /// TODO

    pub const unsafe fn new_edma3(controller: *const ()) -> Self {
        Self {
            controller: ral::Kind::EDma3(ral::Static(controller.cast())),
            wakers: [NO_WAKER; CHANNELS],
        }
    }
}

#[cfg(feature = "edma34")]
impl<const CHANNELS: usize> Dma<4, CHANNELS> {
    /// Create an eDMA3 driver.
    ///
    /// # Safety
    ///
    /// TODO
    #[cfg(feature = "edma34")]
    pub const unsafe fn new_edma4(controller: *const ()) -> Self {
        Self {
            controller: ral::Kind::EDma4(ral::Static(controller.cast())),
            wakers: [NO_WAKER; CHANNELS],
        }
    }
}

impl<const DMA_INST: u8, const CHANNELS: usize> Dma<DMA_INST, CHANNELS> {
    /// Propagate the ID of the bus controller through each DMA channel.
    ///
    /// This is off by default. When it's off, the DMA controller uses nonsecure
    /// privilege to perform transfers.
    ///
    /// When it's on, each DMA channel adopts the domain ID of the bus that
    /// programs the channel. That's likely the CPU's domain ID.
    #[cfg(feature = "edma34")]
    pub unsafe fn set_global_id_replication(&self, enable: bool) {
        match &self.controller {
            ral::Kind::EDma3(edma3) => {
                ral::modify_reg!(ral::dma::edma3, edma3, CSR, GMRC: enable as u32)
            }
            ral::Kind::EDma4(edma4) => {
                ral::modify_reg!(ral::dma::edma4, edma4, CSR, GMRC: enable as u32)
            }
        }
    }
}

use interrupt::{SharedWaker, NO_WAKER};

/// The implementing peripheral works with this DMA controller.
///
/// Implement this on your DMA-capable peripherals to assert that
/// it is compatible with the given `DMA_INST`.
///
/// This trait is safe to implement. An incorrect implementation
/// doesn't violate memory safety; instead, your DMA operation
/// won't work.
pub trait WorksWith<const DMA_INST: u8> {}
