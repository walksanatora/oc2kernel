use crate::println;
use alloc::boxed::Box;
use core::ptr::NonNull;
use virtio_drivers::{BufferDirection, Hal, PhysAddr, PAGE_SIZE};

extern "C" {
    fn end();
}
use once_cell::unsync::OnceCell;

static mut DMA_PADDR: OnceCell<Box<usize>> = OnceCell::new();

pub unsafe fn init_virtio_hal() {
    let _ = DMA_PADDR.set(Box::from(end as usize));
}

pub struct HalImpl;
//TODO: make it so on page dealloc it allows it to be re-allocated instead of just slowly creeping to the right (and possibly causing UB as it creeps into mmio)
unsafe impl Hal for HalImpl {
    fn dma_alloc(pages: usize, _direction: BufferDirection) -> (PhysAddr, NonNull<u8>) {
        let dma = unsafe { DMA_PADDR.get_mut().unwrap() };
        let paddr = **dma;
        **dma += PAGE_SIZE * pages;
        println!("alloc DMA: paddr={:#x}, pages={}", paddr, pages);
        let vaddr = NonNull::new(paddr as _).unwrap();
        (paddr, vaddr)
    }

    unsafe fn dma_dealloc(paddr: PhysAddr, _vaddr: NonNull<u8>, pages: usize) -> i32 {
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
