use bitfield::bitfield;

bitfield! {
  pub(super) struct PciConfig(u32);
  impl Debug;
  // The fields default to u16
  pub enable, set_enable: 31;
  u8; pub bus_number, set_bus_number: 23, 15;
  u8; pub device_number, set_device_number: 15, 11;
  u8; pub function_number, set_function_number: 10, 8;
  u8; pub register_offset, set_register_offset: 7,0 ;
}
