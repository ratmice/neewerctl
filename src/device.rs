#![allow(clippy::upper_case_acronyms)]

use crate::{AnimMode, CCTMode, HSIMode};
use bytemuck::{Pod, Zeroable};
use std::convert::TryFrom;
use std::marker::PhantomData;

mod _errors_ {
    use crate::{AnimMode, CCTMode, HSIMode};
    use std::{error, fmt};
    #[derive(Debug)]
    pub enum ConversionToPacketError {
        CCT(CCTMode),
        HSI(HSIMode),
        Anim(AnimMode),
    }

    impl<'a> fmt::Display for ConversionToPacketError {
        #[rustfmt::skip]
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CCT(cct) => write!(f, "Error converting Mode to packet {:?}", cct),
            Self::HSI(hsi) => write!(f, "Error converting Mode to packet {:?}", hsi),
            Self::Anim(anim) => write!(f, "Error converting Mode to packet {:?}", anim),
        }
    }
    }

    impl error::Error for ConversionToPacketError {
        #[rustfmt::skip]
        fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            ConversionToPacketError::CCT(_) => None,
            ConversionToPacketError::HSI(_) => None,
            ConversionToPacketError::Anim(_) => None,
        }
    }
    }
}

use _errors_::*;

//#[allow(unused)]
#[rustfmt::skip]
mod _characteristics_ {
    use btleplug::api::CharPropFlags as Flags;
    use btleplug::api::Characteristic;
    // Service
    pub const BLE_SVC_UUID_STR: &str       = "69400001-B5A3-F393-E0A9-E50E24DCCA99";
    // W & W w/o Response.
    pub const DEV_CTL_UUID_STR: &str       = "69400002-B5A3-F393-E0A9-E50E24DCCA99";
    // Notify
    pub const GATT_UUID_STR: &str          = "69400003-B5A3-F393-E0A9-E50E24DCCA99";

    // Service
    #[allow(unused)]
    pub const UNKNOWN_SVC_UUID_STR: &str     = "7f510004-b5a3-f393-e0a9-e50e24dcca9e";
    // W & W w/o Response
    #[allow(unused)]
    pub const UNKNOWN_W_WO_UUID_STR: &str    = "7f510005-b5a3-f393-e0a9-e50e24dcca9e";
    // W & W w/o Response & Notify
    #[allow(unused)]
    pub const UNKNOWN_W_WO_NT_UUID_STR: &str = "7f510006-b5a3-f393-e0a9-e50e24dcca9e";
    use lazy_static::lazy_static;
    lazy_static! {
        pub static ref BLE_SVC_UUID: uuid::Uuid = uuid::Uuid::parse_str(BLE_SVC_UUID_STR).unwrap();
        pub static ref DEV_CTL_UUID: uuid::Uuid = uuid::Uuid::parse_str(DEV_CTL_UUID_STR).unwrap();
        pub static ref GATT_UUID: uuid::Uuid = uuid::Uuid::parse_str(GATT_UUID_STR).unwrap();
        pub static ref GATT: Characteristic = 
            Characteristic {
                uuid: *GATT_UUID,
                service_uuid: *BLE_SVC_UUID,
                properties: Flags::NOTIFY,
            };
        pub static ref DEV_CTL: Characteristic =
            Characteristic {
            uuid: *DEV_CTL_UUID,
            service_uuid: *BLE_SVC_UUID,
            properties: Flags::WRITE_WITHOUT_RESPONSE.union(Flags::WRITE),
        };
    }
}
pub use _characteristics_::*;

#[allow(unused)]
#[rustfmt::skip]
mod _in_band_ {
    /// use std::num::Wrapping;
    ///
    /// len == number of u8
    ///
    /// OP            magic?      cmd   len      msg                                     checksum
    ///               --------    ----  ----     ----  -------------------------------------------- 
    /// LIGHT_PWR_ON  [0x78u8,    0x81, 0x01,    0x01, .iter().fold(0u8, |x, y| x.wrapping_add(*y))]
    /// LIGHT_PWR_OFF [0x78u8,    0x81, 0x01,    0x02, checksum]
    ///
    /// ACK?          [0x78u8,    0x84, 0x00, checksum]  responds on Notify service
    /// PWR_STATUS?   [0x78u8,    0x85, 0x00, checksum]  ""
    /// 
    /// // Mode data transimission
    ///  (Checksum excluded, see above)
    /// 
    /// HSI value  type    range
    ///-----------------------------
    ///        hue: u16  |  0..360
    /// saturation: u8   |  0..100  
    /// intensity: u8   |  0..100
    /// [0x78, 0x86, 4, (hue & 0xff) or hue as u8, (hue >> 8) as u8, saturation, intensity, checksum];

    /// CCT
    /// brightness: u8   |  0..100
    /// temp:       u8   |  32..56
    /// [0x78, 0x87, 2, brightness, temp, checksum] 
    
    /// ANIM
    /// brightness: u8   |  0...100
    /// scene:      u8   |  0..9
    /// [0x78, 0x88, 2, brightness, scene, checksum];


    // To dev_ctl
    pub const POWER_ON: [u8; 5] = [0x78, 0x81, 0x01, 0x01, 0xFB];
    pub const POWER_OFF: [u8; 5] = [0x78, 0x81, 0x01, 0x02, 0xFC];
    // To dev_ctl responds on gatt.
    pub const READ_REQUEST: [u8; 4] = [0x78, 0x84, 0x00, 0xFC];
    pub const READ_POWER: [u8; 4] = [0x78, 0x85, 0x00, 0xFD];
}
pub use _in_band_::*;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PacketHeader<T: Packet> {
    pub magic: u8,
    pub cmd: u8,
    pub length: u8,
    marker: PhantomData<T>,
}

unsafe impl<T: Packet> Zeroable for PacketHeader<T> {}
unsafe impl<T: Packet> Pod for PacketHeader<T> {}

impl<T: Packet> Default for PacketHeader<T> {
    fn default() -> PacketHeader<T> {
        PacketHeader {
            magic: T::MAGIC,
            cmd: T::CMD,
            length: T::LENGTH,
            marker: PhantomData,
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PacketFooter<T> {
    pub checksum: u8,
    marker: PhantomData<T>,
}

unsafe impl<T: Copy + 'static> Zeroable for PacketFooter<T> {}
unsafe impl<T: Copy + 'static> Pod for PacketFooter<T> {}

impl<T> Default for PacketFooter<T> {
    fn default() -> PacketFooter<T> {
        PacketFooter {
            checksum: 0,
            marker: PhantomData,
        }
    }
}

// This might be silly, but I was curious about the combination of
// traits with associated constants
pub trait Packet: Sized + Pod + Zeroable + Copy {
    const MAGIC: u8 = 0x78;
    const CMD: u8;
    const LENGTH: u8 = Self::DATA_SIZE as u8;

    const HEADER_SUM: u8 = Self::MAGIC
        .wrapping_add(Self::CMD)
        .wrapping_add(Self::LENGTH);

    // Choice of u8 is because we know this will never exceed that in this protocol.
    // Definitely needs to change for any other.

    const SIZE: usize = std::mem::size_of::<Self>();
    const HEADER_SIZE: usize = std::mem::size_of::<PacketHeader<Self>>();
    const DATA_SIZE: usize = Self::SIZE - (Self::HEADER_SIZE + Self::FOOTER_SIZE);
    const FOOTER_SIZE: usize = std::mem::size_of::<PacketFooter<Self>>();

    const MSG_START: usize = Self::HEADER_SIZE;
    const MSG_END: usize = Self::HEADER_SIZE + Self::DATA_SIZE;

    fn gen_checksum(&self) -> u8 {
        Self::HEADER_SUM.wrapping_add(self.msg_sum())
    }

    fn header(&self) -> &PacketHeader<Self>;
    fn footer(&self) -> &PacketFooter<Self>;
    fn header_mut(&mut self) -> &mut PacketHeader<Self>;
    fn footer_mut(&mut self) -> &mut PacketFooter<Self>;
    fn range_check(&self) -> bool;
    fn prepare(mut self) -> Self {
        {
            let header = self.header_mut();
            header.magic = Self::MAGIC;
            header.cmd = Self::CMD;
            header.length = Self::LENGTH;
        }
        {
            let checksum = self.gen_checksum();
            let mut footer = self.footer_mut();
            footer.checksum = checksum;
        }
        self
    }
    fn bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
    fn msg_sum(&self) -> u8 {
        let bytes = self.bytes();
        let ret: u8 = bytes[Self::MSG_START..Self::MSG_END]
            .iter()
            .fold(0, |x, y| x.wrapping_add(*y));
        ret
    }

    fn check(it: &[u8]) -> bool {
        if it.len() > 3 && it[2] == Self::LENGTH && it[1] == Self::CMD {
            if let Some((checksum, rest)) = it.split_last() {
                if *checksum == rest.iter().fold(0_u8, |x, y| x.wrapping_add(*y)) {
                    return true;
                }
            }
        }
        false
    }
}

// FIXME probably more elegant to use proc macros.
macro_rules! cmd {
    ($cmd:literal) => {
        const CMD: u8 = $cmd;
    };
}

macro_rules! accessors {
    ($header:ident, $footer:ident) => {
        fn header_mut(&mut self) -> &mut PacketHeader<Self> {
            &mut self.$header
        }
        fn footer_mut(&mut self) -> &mut PacketFooter<Self> {
            &mut self.$footer
        }
        fn header(&self) -> &PacketHeader<Self> {
            &self.$header
        }
        fn footer(&self) -> &PacketFooter<Self> {
            &self.$footer
        }
    };
}

macro_rules! packet {
    ($cmd_num:literal) => {
        cmd!($cmd_num);
        accessors!(header, footer);
    };
}

// Fixme internal fields should prefix with _.
#[derive(Pod, Zeroable, Copy, Clone, Debug)]
#[repr(C)]
pub struct Power {
    header: PacketHeader<Power>,
    onoff: u8,
    footer: PacketFooter<Power>,
}

impl Packet for Power {
    packet!(0x81);
    /*
        fn msg_sum(&self) -> u8 {
            self.onoff
        }
    */
    fn range_check(&self) -> bool {
        (1..2).contains(&self.onoff)
    }
}

impl From<bool> for Power {
    fn from(it: bool) -> Power {
        Power {
            onoff: if it { 1u8 } else { 2u8 },
            header: PacketHeader::<Power>::default(),
            footer: PacketFooter::<Power>::default(),
        }
        .prepare()
    }
}

#[derive(Pod, Zeroable, Copy, Clone, Debug)]
#[repr(C)]
pub struct HSI {
    header: PacketHeader<HSI>,
    hue1: u8,
    hue2: u8,
    saturation: u8,
    intensity: u8,
    footer: PacketFooter<HSI>,
}

impl Packet for HSI {
    packet!(0x86);
    fn range_check(&self) -> bool {
        let hue: u16 = (self.hue2 as u16) << 8 | self.hue1 as u16;
        (0..=360).contains(&hue)
            && (0..=100).contains(&self.intensity)
            && (0..=100).contains(&self.saturation)
    }
}

impl TryFrom<HSIMode> for HSI {
    type Error = ConversionToPacketError;
    fn try_from(mode: HSIMode) -> Result<HSI, Self::Error> {
        let intensity = mode.intensity.round() as u8;
        let saturation = mode.saturation.round() as u8;
        let hue: u16 = mode.hue.round() as u16;
        let ret = HSI {
            saturation,
            intensity,
            hue1: (hue & 0xff) as u8,
            hue2: (hue as u16 >> 8) as u8,
            header: PacketHeader::<Self>::default(),
            footer: PacketFooter::<Self>::default(),
        };
        if ret.range_check() {
            Ok(ret.prepare())
        } else {
            Err(Self::Error::HSI(mode))
        }
    }
}

#[derive(Pod, Zeroable, Copy, Clone, Debug)]
#[repr(C)]
pub struct CCT {
    header: PacketHeader<CCT>,
    brightness: u8,
    temp: u8,
    footer: PacketFooter<CCT>,
}

impl Packet for CCT {
    packet!(0x87);
    fn range_check(&self) -> bool {
        (32..=56).contains(&self.temp) && (0..=100).contains(&self.brightness)
    }
}

impl TryFrom<CCTMode> for CCT {
    type Error = ConversionToPacketError;
    fn try_from(mode: CCTMode) -> Result<CCT, Self::Error> {
        let brightness = mode.brightness.round() as u8;
        let temp = mode.temp.round() as u8;
        let ret = CCT {
            temp,
            brightness,
            header: PacketHeader::<Self>::default(),
            footer: PacketFooter::<Self>::default(),
        };
        if ret.range_check() {
            Ok(ret.prepare())
        } else {
            Err(Self::Error::CCT(mode))
        }
    }
}

#[derive(Pod, Zeroable, Copy, Clone, Debug)]
#[repr(C)]
pub struct Anim {
    header: PacketHeader<Anim>,
    brightness: u8,
    scene: u8,
    footer: PacketFooter<Anim>,
}

impl Packet for Anim {
    packet!(0x88);
    fn range_check(&self) -> bool {
        (0..=9).contains(&self.scene) && (0..=100).contains(&self.brightness)
    }
}

impl TryFrom<AnimMode> for Anim {
    type Error = ConversionToPacketError;
    fn try_from(mode: AnimMode) -> Result<Anim, Self::Error> {
        let brightness = mode.brightness.round() as u8;
        let scene = mode.scene.round() as u8;
        let ret = Anim {
            brightness,
            scene,
            header: PacketHeader::<Self>::default(),
            footer: PacketFooter::<Self>::default(),
        };
        if ret.range_check() {
            Ok(ret.prepare())
        } else {
            Err(Self::Error::Anim(mode))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_power_on() {
        let val = Power::from(true).prepare();
        println!("{:?} {:?}", val.bytes(), POWER_ON);
        assert_eq!(val.bytes(), POWER_ON);
    }
    #[test]
    fn test_power_off() {
        let val = Power::from(false).prepare();
        assert_eq!(val.bytes(), POWER_OFF);
        println!("{:?} {:?}", val.bytes(), POWER_OFF);
    }
    #[test]
    fn test_hsi1() -> Result<(), ConversionToPacketError> {
        let hsi = HSIMode {
            hue: 283.0,
            saturation: 100.0,
            intensity: 6.0,
        };
        let dev_hsi: HSI = TryFrom::try_from(hsi)?;
        let bytes = dev_hsi.bytes();

        assert_eq!(283, (bytes[3] as u16) | (bytes[4] as u16) << 8);
        assert_eq!(100, bytes[5]);
        assert_eq!(6, bytes[6]);
        Ok(())
    }
}
