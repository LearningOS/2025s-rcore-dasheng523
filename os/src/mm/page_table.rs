//! Implementation of [`PageTableEntry`] and [`PageTable`].

use crate::config::PAGE_SIZE;

use super::{frame_alloc, FrameTracker, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use alloc::vec;
use alloc::vec::Vec;
use bitflags::*;

bitflags! {
    /// page table entry flags
    pub struct PTEFlags: u8 {
        /// Valid
        const V = 1 << 0;
        /// Readable
        const R = 1 << 1;
        /// Writable
        const W = 1 << 2;
        /// eXecutable
        const X = 1 << 3;
        /// User
        const U = 1 << 4;
        /// Global
        const G = 1 << 5;
        /// Accessed
        const A = 1 << 6;
        /// Dirty
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
/// page table entry structure
pub struct PageTableEntry {
    /// bits of page table entry
    pub bits: usize,
}

impl PageTableEntry {
    /// Create a new page table entry
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }
    /// Create an empty page table entry
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }
    /// Get the physical page number from the page table entry
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10 & ((1usize << 44) - 1)).into()
    }
    /// Get the flags from the page table entry
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }
    /// The page pointered by page table entry is valid?
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is readable?
    pub fn readable(&self) -> bool {
        (self.flags() & PTEFlags::R) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is writable?
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }
    /// The page pointered by page table entry is executable?
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

/// page table structure
pub struct PageTable {
    root_ppn: PhysPageNum,
    frames: Vec<FrameTracker>,
}

/// Assume that it won't oom when creating/mapping.
impl PageTable {
    /// Create a new page table
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }
    /// Temporarily used to get arguments from user space.
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }
    /// Find PageTableEntry by VirtPageNum, create a frame for a 4KB page table if not exist
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }
    /// Find PageTableEntry by VirtPageNum
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }
    /// set the map between virtual page number and physical page number
    #[allow(unused)]
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }
    /// remove the map between virtual page number and physical page number
    #[allow(unused)]
    pub fn unmap(&mut self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }
    /// get the page table entry from the virtual page number
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }
    /// get the token from the page table
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

/// Translate&Copy a ptr[u8] array with LENGTH len to a mutable u8 Vec through page table
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.page_offset() == 0 {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}


/// 从用户空间读取数据并转换为指定类型
/// 
/// # 参数
/// 
/// * `token` - 页表标识符
/// * `ptr` - 用户空间源地址的指针
/// 
/// # 返回值
/// 
/// * `Some(T)` - 读取并转换成功后的数据
/// * `None` - 读取失败（源地址无效或不可读）
/// 
/// # 泛型参数
/// 
/// * `T` - 要转换成的目标类型
pub fn translate_data<T: core::fmt::Debug>(token: usize, ptr: *const u8) -> Option<T> {
    let page_table = PageTable::from_token(token);
    let start = ptr as usize;
    let start_va = VirtAddr::from(start);
    let vpn: VirtPageNum = start_va.floor();
    let entry = page_table.translate(vpn).unwrap();
    if !entry.is_valid() || !entry.readable() {
        return None;
    }

    let data_len = core::mem::size_of::<T>();
    let buffer = translated_byte_buffer(token, ptr, data_len);
    println!("my buffer: {:?}", buffer);

    // 创建一个未初始化的T实例
    let mut data: T = unsafe { core::mem::MaybeUninit::uninit().assume_init() };
    
    // 将buffer中的数据复制到data中
    unsafe {
        let data_ptr = &mut data as *mut T as *mut u8;
        let mut offset = 0;
        for slice in buffer {
            core::ptr::copy_nonoverlapping(
                slice.as_ptr(),
                data_ptr.add(offset),
                slice.len()
            );
            offset += slice.len();
        }
    }
    //println!("my data: {:?}", data);
    // return Some(data);
    return None
} 


/// 将数据写入用户空间
/// 
/// # 参数
/// 
/// * `token` - 页表标识符
/// * `ptr` - 用户空间目标地址的指针
/// * `data` - 要写入的数据的指针
/// 
/// # 返回值
/// 
/// * `1` - 写入成功
/// * `-1` - 写入失败（目标地址无效或不可写）
pub fn write_data<T>(token: usize, ptr: *mut u8, data: *const T) -> isize {
    let page_table = PageTable::from_token(token);
    let start = ptr as usize;
    let start_va = VirtAddr::from(start);
    let vpn: VirtPageNum = start_va.floor();
    let entry = page_table.translate(vpn).unwrap();
    if !entry.is_valid() || !entry.writable() {
        return -1;
    }

    let data_len = core::mem::size_of::<T>();
    let mut data_u8 = vec![0u8; data_len];
    unsafe {
        let data_ptr = data as *const u8;
        core::ptr::copy_nonoverlapping(data_ptr, data_u8.as_mut_ptr(), data_len);
    }
    write_byte_buffer(token, ptr as *mut u8, &data_u8);
    return 1;
} 

/// Write data to user space through page table
/// 
/// # Arguments
/// 
/// * `token` - The page table token
/// * `ptr` - Pointer to the destination in user space
/// * `data` - The data to be written
pub fn write_byte_buffer(token: usize, ptr: *mut u8, data: &[u8]) {
    let page_table = PageTable::from_token(token);
    let start = ptr as usize;
    let start_va = VirtAddr::from(start);
    let mut vpn = start_va.floor();
    let ppn = page_table.translate(vpn).unwrap().ppn();

    // copy time to ppn
    // 先看看会不会跨页
    if start_va.page_offset() + data.len() <= PAGE_SIZE {
        let dst = &mut ppn.get_bytes_array()[start_va.page_offset()..start_va.page_offset() + data.len()];
        dst.copy_from_slice(data);
    }
    else {
        // 先复制第一页中的部分
        let first_page_bytes = PAGE_SIZE - start_va.page_offset();
        let dst_first = &mut ppn.get_bytes_array()[start_va.page_offset()..];
        dst_first.copy_from_slice(&data[..first_page_bytes]);
        
        // 处理第二页
        vpn.step();
        let ppn_next = page_table.translate(vpn).unwrap().ppn();
        let dst_second = &mut ppn_next.get_bytes_array()[..data.len() - first_page_bytes];
        dst_second.copy_from_slice(&data[first_page_bytes..]);
    }
}
