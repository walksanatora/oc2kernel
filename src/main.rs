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

use core::{panic::PanicInfo, ptr::write_volatile};

static mut UART_BASE: *mut u8 = 0x1000_0148 as *mut u8;

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
    "0:  bgeu    t0, t1, 1f",
        "sb      zero, (t0)",
        "addi    t0, t0, 1",
        "j       0b",
    "1:",

        // "tail-call" to {entry} (call without saving a return address)
        "tail {entry}",
        entry = sym entry, // {entry} refers to the function [entry] below
        options(noreturn) // we must handle "returning" from assembly
      );
}

extern "C" fn entry(_hard_id: u64, fdt_ptr: *const u8) -> ! {
    unsafe {
        //#region UART setup
        let dev_tree = fdt::Fdt::from_ptr(fdt_ptr).unwrap();

        UART_BASE = dev_tree
            .chosen()
            .stdout()
            .unwrap()
            .reg()
            .unwrap()
            .next()
            .unwrap()
            .starting_address
            .cast_mut();

        // Set data size to 8 bits.
        write_volatile(UART_BASE.offset(3), 0b11);
        // Enable FIFO.
        write_volatile(UART_BASE.offset(2), 0b1);
        // Enable receiver buffer interrupts.
        write_volatile(UART_BASE.offset(1), 0b1);
        //#endregion

        println!("hello!");
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
