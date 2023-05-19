#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
#![feature(panic_info_message)]

//allocator so that we can use alloc variables
use simple_chunk_allocator::{heap, heap_bitmap, GlobalChunkAllocator, PageAligned};
static mut HEAP: PageAligned<[u8; 2097152]> = heap!(chunks = 2048, chunksize = 1024);
static mut HEAP_BITMAP: PageAligned<[u8; 1024]> = heap_bitmap!(chunks = 8192);
#[global_allocator]
static ALLOCATOR: GlobalChunkAllocator =
    unsafe { GlobalChunkAllocator::new(HEAP.deref_mut_const(), HEAP_BITMAP.deref_mut_const()) };

//globals that we init on start so that we can use them anywhere
// once cell? (probally should)
static mut UART_BASE: *mut u8 = 0x1000_0148 as *mut u8;
static mut DEVICE_TREE_PTR: *const u8 = 0x0 as *const u8;

//imports
//basic rust things
extern crate alloc;
use crate::uart::init_from_mmio;
use core::panic::PanicInfo;

//entrypoint
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

        //zero-out .bss as the rusty overlords expect
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

/// now we can start cooking, our real code exist here
extern "C" fn entry(_hard_id: u64, fdt_ptr: *const u8) -> ! {
    unsafe {
        //init the virtio hal as `lazy_static` doesen't exist
        virtio_hal::init_virtio_hal();

        //setup the globals
        DEVICE_TREE_PTR = fdt_ptr; //device tree ptr
        let dev_tree = fdt::Fdt::from_ptr(fdt_ptr).unwrap();
        //get Uart base addr
        UART_BASE = dev_tree //uart base ptr
            .chosen()
            .stdout()
            .unwrap()
            .reg()
            .unwrap()
            .next()
            .unwrap()
            .starting_address
            .cast_mut();

        init_from_mmio(UART_BASE as usize); //setup the UART for terminal output

        //the real program
        println!();
        println!("Hello, World");
        println!("cpu count: {}", dev_tree.cpus().count());
        println!(
            //number of virtio devices
            "virtio devs: {}",
            dev_tree.find_all_nodes("/virtio").count()
        );
        for virt in dev_tree.find_all_nodes("/virtio") {
            //print virtio devices + device IDs
            print!("virt: {} d:", virt.name);
            let device = virt
                .reg()
                .unwrap()
                .next()
                .unwrap()
                .starting_address
                .offset(8) as *const u32;
            println!("{}", *device);
        }
        println!("goodbye");
        panic!("reached end of program")
    }
}

#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    let file = location.file();
    let line = location.line();
    print!("{}: ", file); //panic only the file and line
    println!("{}", line);
    println!("{:?}", info); // and print the full panic object just cause
    loop {}
}

mod plic;
mod uart;
mod virtio_hal;
