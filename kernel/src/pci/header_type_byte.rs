use bitfield::bitfield;

bitfield! {
  pub struct HeaderTypeByte(u8);
  impl Debug;
  // The fields default to u16
  pub multi_function, _: 7;
  u8; pub header_type, _: 6, 0;
}
