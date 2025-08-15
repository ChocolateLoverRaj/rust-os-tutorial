use core::{fmt::Debug, marker::PhantomData, num::NonZero, ptr::NonNull};

use bitfield::bitfield;
use volatile::{VolatileFieldAccess, VolatileRef};

use crate::capability_regs::{CapabilityRegs, CapabilityRegsVolatileFieldAccess};

#[derive(Clone)]
pub struct XhciExtendedCapabilities<'a> {
    _phantom_data: PhantomData<&'a ()>,
    bar0: NonZero<usize>,
}

impl XhciExtendedCapabilities<'_> {
    /// # Safety
    /// This function will read the capability registers and read extended capability registers.
    pub unsafe fn new(bar0: NonZero<usize>) -> Self {
        Self {
            _phantom_data: PhantomData,
            bar0,
        }
    }
}

impl<'a> IntoIterator for XhciExtendedCapabilities<'a> {
    type IntoIter = XhciExtendedCapabilitiesIterator<'a>;
    type Item = XhciExtendedCapability<'a>;

    fn into_iter(self) -> Self::IntoIter {
        XhciExtendedCapabilitiesIterator::new(self)
    }
}

pub struct XhciExtendedCapabilitiesIterator<'a> {
    capabilities: XhciExtendedCapabilities<'a>,
    /// An offset in `u32`s from the base
    ptr: Option<NonZero<u16>>,
}

impl<'a> XhciExtendedCapabilitiesIterator<'a> {
    fn new(capabilities: XhciExtendedCapabilities<'a>) -> Self {
        Self {
            ptr: {
                let capability_regs = {
                    let capability_regs_ptr =
                        NonNull::new(capabilities.bar0.get() as *mut CapabilityRegs).unwrap();
                    unsafe { VolatileRef::new(capability_regs_ptr) }
                };
                NonZero::new(
                    capability_regs
                        .as_ptr()
                        .hcc_params_1()
                        .read()
                        .xhci_extended_capabilities_ptr(),
                )
            },
            capabilities,
        }
    }
}

impl<'a> Iterator for XhciExtendedCapabilitiesIterator<'a> {
    type Item = XhciExtendedCapability<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let offset_u32s = self.ptr?;
        let ptr = NonNull::new(
            (self.capabilities.bar0.get() + offset_u32s.get() as usize * size_of::<u32>())
                as *mut XhciExtendedCapabilityPointerRegister,
        )
        .unwrap();
        let register = unsafe { VolatileRef::new(ptr) };
        self.ptr = NonZero::new(register.as_ptr().read().next_xhci_extended_capability_ptr()).map(
            |relative_offset| {
                offset_u32s
                    .checked_add(relative_offset.get() as u16)
                    .expect("offset does not overflow")
            },
        );
        Some(XhciExtendedCapability { register })
    }
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct XhciExtendedCapabilityPointerRegister(u32);
    impl Debug;

    u8; pub capability_id, _: 7, 0;
    u8; next_xhci_extended_capability_ptr, _: 15, 8;
    u16; pub capability_specific, _: 31, 16;
}

pub struct XhciExtendedCapability<'a> {
    register: VolatileRef<'a, XhciExtendedCapabilityPointerRegister>,
}

impl XhciExtendedCapability<'_> {
    pub fn supported_protocol(&self) -> Option<XhciSupportedProtocolCapability> {
        if self.register.as_ptr().read().capability_id() == 0x2 {
            Some({
                let ptr = self
                    .register
                    .as_ptr()
                    .as_raw_ptr()
                    .cast::<XhciSupportedProtocolCapability>();
                unsafe { ptr.read_volatile() }
            })
        } else {
            None
        }
    }
}

/// 7.2 xHCI Supported Protocol Capability
#[derive(Debug, VolatileFieldAccess)]
#[repr(C)]
pub struct XhciSupportedProtocolCapability {
    _00: XhciSupportedProtocolCapability00,
    name_string: u32,
    _08: XhciSupportedProtocolCapability08,
    // There is technically more after this, but we don't really care
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct XhciSupportedProtocolCapability00(u32);
    impl Debug;

    u8; capability_id, _: 7, 0;
    u8; next_capability_pointer, _: 15, 8;
    u8; revision_minor, _: 23, 16;
    u8; revision_major, _: 31, 24;
}

bitfield! {
    #[derive(Clone, Copy)]
    pub struct XhciSupportedProtocolCapability08(u32);
    impl Debug;

    u8; pub compatible_port_offset, _: 7, 0;
    u8; pub compatible_port_count, _: 15, 8;
    u16; pub protocol_defined, _: 27, 16;
    u8; pub protocol_speed_id_count, _: 31, 28;
}
