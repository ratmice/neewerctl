use btleplug::platform::PeripheralId;
use druid::{Data, Lens};
use druid_widget_nursery::prism::Prism;
use enum_extra::{Mask, NonZeroRepr, OpaqueRepr};
use std::cmp::{Eq, PartialEq};
use std::collections::BTreeMap;
use std::iter::Iterator;
use std::sync::Arc;
use strum::EnumMetadata;

/// AppState...
#[derive(Clone, Data, Debug, Lens)]
pub struct AppState {
    pub(crate) lights: Arc<BTreeMap<PeripheralId, Light>>,
    pub(crate) scanning: bool,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            lights: Arc::new(BTreeMap::new()),
            scanning: false,
        }
    }
}

impl AppState {
    pub fn toggle_scanning(&mut self) {
        self.scanning = !self.scanning;
    }
}

/// Light...
#[derive(Clone, Data, Lens, Debug, PartialEq)]
pub struct Light {
    pub(crate) mode: LightMode,
    pub(crate) power: bool,
    pub(crate) connected: bool,
    #[data(ignore)]
    pub(crate) _changes_: OpaqueRepr<Changed>,
}

#[derive(Data, Clone, Debug, Eq, PartialEq, EnumMetadata, NonZeroRepr)]
#[repr(u8)]
pub enum Changed {
    Mode = 1 << 0,
    Power = 1 << 1,
    // This should perhaps split into GuiChanged and DeviceChanged.
    // For now...
    Connected = 1 << 2,
}

#[cfg(test)]
mod test {
    use super::*;
    use enum_extra::OpaqueMetadata;
    #[test]
    fn change_iter() {
        assert_eq!(
            OpaqueRepr::<Changed>::zero()
                .mask_iter()
                .collect::<Vec<Changed>>(),
            vec![]
        );
        assert_eq!(
            Changed::Mode
                .opaque_repr()
                .mask_iter()
                .collect::<Vec<Changed>>(),
            vec![Changed::Mode]
        );
        assert_eq!(
            Changed::Power
                .opaque_repr()
                .mask_iter()
                .collect::<Vec<Changed>>(),
            vec![Changed::Power]
        );
        assert_eq!(
            (Changed::Mode.opaque_repr() | Changed::Power)
                .mask_iter()
                .collect::<Vec<Changed>>(),
            vec![Changed::Mode, Changed::Power]
        );
    }
}

impl Default for Light {
    fn default() -> Light {
        Light {
            mode: LightMode::CCT(CCTMode {
                temp: 32.0,
                brightness: 0.0,
            }),
            power: false,
            connected: false,
            _changes_: OpaqueRepr::<Changed>::zero(),
        }
    }
}

impl Light {
    pub fn clear_changes(&mut self) {
        self._changes_ = OpaqueRepr::<Changed>::zero();
    }
    pub fn has_changes(&self) -> bool {
        self._changes_ != OpaqueRepr::<Changed>::zero()
    }
    pub fn changes(&self) -> impl Iterator<Item = Changed> {
        self._changes_.mask_iter()
    }

    pub fn toggle_power(&mut self) {
        self.power = !self.power;
        self._changes_ |= Changed::Power;
    }

    pub fn sync(&mut self, _other: &Self) {
        // FIXME..
    }
}

/// LightMode...
#[derive(Clone, Data, Debug, PartialEq, Prism)]
#[allow(clippy::upper_case_acronyms)]
pub enum LightMode {
    CCT(CCTMode),
    HSI(HSIMode),
    Anim(AnimMode),
}

#[derive(Clone, Data, Debug, Lens, PartialEq)]
pub struct CCTMode {
    pub temp: f64,
    pub brightness: f64,
}

#[derive(Clone, Data, Debug, Lens, PartialEq)]
pub struct HSIMode {
    pub hue: f64,
    pub saturation: f64,
    pub intensity: f64,
}

#[derive(Clone, Data, Debug, Lens, PartialEq)]
pub struct AnimMode {
    pub scene: f64,
    pub brightness: f64,
}
