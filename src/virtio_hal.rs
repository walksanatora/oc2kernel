use crate::println;
use core::{
    ptr::{self, NonNull},
    sync::atomic::{AtomicU64, AtomicU8, Ordering},
};
use virtio_drivers::{BufferDirection, Hal, PhysAddr, PAGE_SIZE};

extern "C" {
    fn end();
}
/// a bitfield representing which pages have not been allocated
pub static OPEN_PAGES: AtomicU64 = AtomicU64::new(0);
/// the number of allocated pages
pub static ALLOC_PAGES: AtomicU8 = AtomicU8::new(0);

///currently no special init needs to be done (used to have to init a pointer to the end symbol above)
pub fn init_virtio_hal() {}

///generate a mask with the left-most bits being set to 1
fn generate_mask(n: usize) -> u64 {
    assert!(n <= 64, "gen_mask: n must be less than or equal to 64");
    if n == 0 {
        0
    } else {
        u64::MAX << (64 - n)
    }
}

///searches for a section of "pages" size that is all unallocated
pub fn find_open_pages(pages: usize) -> usize {
    let pages_bitfield = OPEN_PAGES.load(Ordering::Relaxed);
    if (pages > 64) || (ALLOC_PAGES.load(Ordering::Relaxed) >= 64) {
        panic!(
            "why are more than 64 pages being allocated {}",
            if pages > 64 { "at once" } else { "" },
        )
    }
    let mask = generate_mask(pages);
    let mut offset: usize = 0;
    let mut found: bool = false;
    for off in 0..64 - pages {
        let omask = mask >> off;
        if (pages_bitfield & omask) == 0 {
            found = true;
            offset = off;
            break;
        }
    }
    if !found {
        panic!(
            "unnable to find open slot of size {}, {:64b}",
            pages, pages_bitfield
        )
    }
    offset
}

/// used to clear the memory once the page is de-allocated
unsafe fn zero_out_memory(ptr: *mut u8, count: usize) {
    let start = ptr;
    let end = start.add(count);
    let mut current = start;

    while current < end {
        ptr::write_volatile(current, 0);
        current = current.offset(1);
    }
}

pub struct HalImpl;
//TODO: make it so on page dealloc it allows it to be re-allocated instead of just slowly creeping to the right (and possibly causing UB as it creeps into mmio)
unsafe impl Hal for HalImpl {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let block_offset = find_open_pages(pages);
        let dma_block = (end as usize) + (PAGE_SIZE * block_offset);
        ALLOC_PAGES.fetch_add(pages as u8, Ordering::Relaxed);
        OPEN_PAGES.fetch_or(generate_mask(pages) >> block_offset, Ordering::Relaxed);
        let vaddr = NonNull::new(dma_block as _).unwrap();
        #[cfg(debug_assertions)]
        println!(
            "alloc@{}?{}: {:064b}",
            block_offset,
            pages,
            OPEN_PAGES.load(Ordering::Relaxed)
        );
        println!(
            "{} + ({} * {}) = {}",
            end as usize, PAGE_SIZE, block_offset, dma_block
        );
        (dma_block, vaddr)
    }

    unsafe fn dma_dealloc(paddr: PhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
        println!("dealloc");
        let offset = (paddr - (end as usize)) / PAGE_SIZE;
        let negate = !(generate_mask(pages) >> offset);
        OPEN_PAGES.fetch_and(negate, Ordering::Relaxed);
        ALLOC_PAGES.fetch_sub(pages as u8, Ordering::Relaxed);
        zero_out_memory(paddr as *mut u8, pages * PAGE_SIZE);
        println!("dealloc DMA: paddr={:#x}, pages={}", paddr, pages);
        0
    }

    unsafe fn mmio_phys_to_virt(paddr: PhysAddr, _size: usize) -> NonNull<u8> {
        NonNull::new(paddr as _).unwrap()
    }

    unsafe fn share(buffer: NonNull<[u8]>, _direction: BufferDirection) -> PhysAddr {
        let vaddr = buffer.as_ptr() as *mut u8 as usize;
        // Nothing to do, as the host already has access to all memory.
        virt_to_phys(vaddr)
    }

    unsafe fn unshare(_paddr: PhysAddr, _buffer: NonNull<[u8]>, _direction: BufferDirection) {
        // Nothing to do, as the host already has access to all memory and we didn't copy the buffer
        // anywhere else.
    }
}

fn virt_to_phys(vaddr: usize) -> PhysAddr {
    vaddr
}
