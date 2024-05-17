use crate::mm::type_cast::MapPermission;

pub mod virt_block;

pub const MAP_PERMISSION_RW: MapPermission = MapPermission::union(MapPermission::R, MapPermission::W);

pub const MMIO: &[(&str, usize, usize, MapPermission)] = &[
    ("VirtIO", 0x1000_1000, 0x1000, MAP_PERMISSION_RW),
];