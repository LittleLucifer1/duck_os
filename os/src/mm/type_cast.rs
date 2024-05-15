//！ 各种bits类型的转换，统一放在这个模块，从而保证可见性。

use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct MapPermission: u8 {
        const R = 1 << 0;
        const W = 1 << 1;
        const X = 1 << 2;
        const U = 1 << 3;
    }
}


bitflags! {
    #[derive(Debug)]
    pub struct MmapProt: u32 {
        const PROT_NONE = 0;
        const PROT_READ = 1 << 0;
        const PROT_WRITE = 1 << 1;
        const PROT_EXEC = 1 << 2;
    }
}


bitflags! {
    #[derive(Debug)]
    pub struct MmapFlags: u32 {
        const MAP_SHARED = 1 << 0;
        const MAP_PRIVATE = 1 << 1;
        const MAP_FIXED = 1 << 4;
        const MAP_ANONYMOUS = 1 << 5;
        const MAP_STACK = 1 << 17;
    }
}


// 0 ~ 9 V:0, R:1, W:2, X:3, U:4, G:5, A:6, D:7, RSW:8~9
bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct PTEFlags: u16 {
        const V = 1 << 0;
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
        const G = 1 << 5;
        const A = 1 << 6;
        const D = 1 << 7;
        const COW = 1 << 8;
    }
}

// TODO： 这种不同映射的转换还没有想清楚，待解决！！
// Unsafe: 还有就是一个bug：from_bit(0) != empty()
//！
bitflags! {
    #[derive(Clone, Copy)]
    pub struct PagePermission: u8 {
        const R = 1 << 1;
        const W = 1 << 2;
        const X = 1 << 3;
        const U = 1 << 4;
    }
}

impl From<MapPermission> for PagePermission {
    fn from(flags: MapPermission) -> Self {
        let mut page_permission = PagePermission::empty();
        if flags.contains(MapPermission::R) {
            page_permission |= PagePermission::R;
        }
        if flags.contains(MapPermission::W) {
            page_permission |= PagePermission::W;
        }
        if flags.contains(MapPermission::X) {
            page_permission |= PagePermission::X;
        }
        if flags.contains(MapPermission::U) {
            page_permission |= PagePermission::U;
        }
        page_permission
    }
}

impl From<MapPermission> for PTEFlags {
    fn from(value: MapPermission) -> Self {
        let mut flags = PTEFlags::empty();
        if value.contains(MapPermission::R) {
            flags |= PTEFlags::R;
        }
        if value.contains(MapPermission::W) {
            flags |= PTEFlags::W;
        }
        if value.contains(MapPermission::X) {
            flags |= PTEFlags::X;
        }
        if value.contains(MapPermission::U) {
            flags |= PTEFlags::U;
        }
        flags
    }
}

impl From<MmapProt> for MapPermission {
    fn from(value: MmapProt) -> Self {
        let mut per = MapPermission::empty();
        if value.contains(MmapProt::PROT_EXEC) {
            per |= MapPermission::X;
        }
        if value.contains(MmapProt::PROT_WRITE) {
            per |= MapPermission::W;
        }
        if value.contains(MmapProt::PROT_READ) {
            per |= MapPermission::R;
        }
        // TODO: 这个参数不太好说，需要仔细的去看相关的代码 这个参数是pages may not be accessed.
        if !value.contains(MmapProt::PROT_NONE) {
            per |= MapPermission::U;
        }
        per
    }
}

impl From<PagePermission> for PTEFlags {
    fn from(value: PagePermission) -> Self {
        let mut flags = PTEFlags::empty();
        if value.contains(PagePermission::R) {
            flags |= PTEFlags::R;
        }
        if value.contains(PagePermission::W) {
            flags |= PTEFlags::W;
        }
        if value.contains(PagePermission::X) {
            flags |= PTEFlags::X;
        }
        if value.contains(PagePermission::U) {
            flags |= PTEFlags::U;
        }
        flags
    }
}