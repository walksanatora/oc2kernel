#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
extern crate alloc;

//alloc setup
use simple_chunk_allocator::{heap, heap_bitmap, GlobalChunkAllocator, PageAligned};
static mut HEAP: PageAligned<[u8; 1048576]> = heap!();
static mut HEAP_BITMAP: PageAligned<[u8; 512]> = heap_bitmap!();
#[global_allocator]
static ALLOCATOR: GlobalChunkAllocator =
    unsafe { GlobalChunkAllocator::new(HEAP.deref_mut_const(), HEAP_BITMAP.deref_mut_const()) };

use core::panic::PanicInfo;

#[naked]
#[no_mangle]
#[link_section = ".text.init"]
unsafe extern "C" fn _start() -> ! {
    use core::arch::asm;
    asm!(
      // before we use the `la` pseudo-instruction for the first time,
      //  we need to set `gp` (google linker relaxation)
      ".option push",
      ".option norelax",
      "la gp, _global_pointer",
      ".option pop",

      // set the stack pointer
      "la sp, _init_stack_top",

      "lla     t0, _bss_start",
      "lla     t1, _bss_end",
      "0:",
      "sb      zero, (t0)",
      "addi    t0, t0, 1",
      "bltu    t0, t1, 0b",

      // "tail-call" to {entry} (call without saving a return address)
      "tail {entry}",
      entry = sym entry, // {entry} refers to the function [entry] below
      options(noreturn) // we must handle "returning" from assembly
    );
}

extern "C" fn entry(_hard_id: u64, fdt_ptr: *const u8) -> ! {
    unsafe {
        let dev_tree = fdt::Fdt::from_ptr(fdt_ptr).unwrap();
        println!("Hello, World");
        println!("cpu count: {:?}", dev_tree.cpus().count());
        panic!("reached end of program")
    }
}

#[panic_handler]
fn on_panic(_info: &PanicInfo) -> ! {
    println!("{:?}", _info);
    loop {}
}

mod uart;
