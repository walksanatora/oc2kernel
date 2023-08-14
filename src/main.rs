#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
#![feature(panic_info_message)]

//allocator so that we can use alloc variables
use simple_chunk_allocator::{heap, heap_bitmap, GlobalChunkAllocator, PageAligned};
use uart::UartLogger;
use virtio_drivers::{
    device::blk::{BlkReq, BlkResp, VirtIOBlk, SECTOR_SIZE},
    transport::{
        mmio::{MmioTransport, VirtIOHeader},
        DeviceType, Transport,
    },
};
static mut HEAP: PageAligned<[u8; 2097152]> = heap!(chunks = 2048, chunksize = 1024);
static mut HEAP_BITMAP: PageAligned<[u8; 1024]> = heap_bitmap!(chunks = 8192);
#[global_allocator]
static ALLOCATOR: GlobalChunkAllocator =
    unsafe { GlobalChunkAllocator::new(HEAP.deref_mut_const(), HEAP_BITMAP.deref_mut_const()) };

//globals that we init on start so that we can use them anywhere
// once cell? (probally should)
static mut UART_BASE: *mut u8 = 0x1000_0148 as *mut u8;
static mut DEVICE_TREE_PTR: *const u8 = 0x0 as *const u8;
static LOGGER: UartLogger = UartLogger {};
//imports
//basic rust things
extern crate alloc;
use crate::{
    uart::init_from_mmio,
    virtio_hal::{HalImpl, ALLOC_PAGES, OPEN_PAGES},
};
use core::{hint::spin_loop, panic::PanicInfo, ptr::NonNull, sync::atomic::Ordering};

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
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Trace);
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

        #[cfg(debug_assertions)]
        for dev in dev_tree.all_nodes() {
            println!("name: {}, ", dev.name);
            if let Some(compat) = dev.compatible() {
                for comp in compat.all() {
                    println!("\tCompat: {}", comp);
                }
            }
        }
        println!("getting vio blk");
        //get Virtio block device
        let vio_blk = dev_tree
            .find_compatible(&["virtio,mmio"])
            .expect("wheres a disk drive");
        println!("vio header");
        let vio_hdr = NonNull::new(
            vio_blk.reg().unwrap().next().unwrap().starting_address as *mut VirtIOHeader,
        )
        .unwrap();
        println!("creating transport");
        //create a virtio transport
        let vio_trans = MmioTransport::new(vio_hdr).unwrap();
        println!("ttdt: {:?}", vio_trans.device_type());
        //turn it into a block
        println!("casting to block");
        let mut vio_block = VirtIOBlk::<HalImpl, _>::new(vio_trans).expect("not a block");
        println!("casted");
        //get more info
        println!("pages: {:?}", vio_block.capacity());
        println!("Read Only: {}", vio_block.readonly());
        //HERE WE GO
        let mut hi_bytes: [u8; SECTOR_SIZE] = [0u8; SECTOR_SIZE];
        hi_bytes[..12].copy_from_slice(b"Hello World?");
        vio_block
            .write_blocks(1, &hi_bytes)
            .expect("failed to write");
        panic!("Temp End!!");
        let mut _blk = None;
        for virt in dev_tree.all_nodes().filter(|node| {
            if let Some(compat) = node.compatible() {
                compat.all().filter(|i| i == &"virtio").count() > 0
            } else {
                false
            }
        }) {
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
            if *device == 2 {
                //we got ourselfes a storage device
                let header = NonNull::new(
                    virt.reg().unwrap().next().unwrap().starting_address as *mut VirtIOHeader,
                )
                .unwrap();
                let transport = MmioTransport::new(header).unwrap();
                if transport.device_type() != DeviceType::Block {
                    panic!("I THOUGHT ID 2 WAS A BLOCK DEVICE")
                }
                _blk = Some(VirtIOBlk::<HalImpl, _>::new(transport).unwrap());
                let mut ublk = _blk.unwrap();
                println!("VirtIO block mounted:");
                let blcks = ublk.capacity();
                println!("Blocks: {}", blcks);
                println!("Size: {} bytes", (blcks * (SECTOR_SIZE as u64)));
                println!("Read Only: {}", ublk.readonly());
                let mut hi_bytes: [u8; SECTOR_SIZE] = [0u8; SECTOR_SIZE];
                hi_bytes[..12].copy_from_slice(b"Hello World!");
                let mut req = BlkReq::default();
                let mut resp = BlkResp::default();
                let token = ublk
                    .write_blocks_nb(0, &mut req, &hi_bytes, &mut resp)
                    .unwrap();
                println!("token is: {}", token);
                loop {
                    if ublk.ack_interrupt() {
                        break;
                    }
                    spin_loop()
                }
                println!(
                    "complete write {:?}",
                    ublk.complete_write_blocks(token, &req, &hi_bytes, &mut resp)
                );

                break;
            }
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
    print!("{}@", file); //panic only the file and line
    print!("{}: ", line);
    let tmp = format_args!("");
    println!("{}", info.message().unwrap_or(&tmp));
    println!("DMA pages: {:064b}", OPEN_PAGES.load(Ordering::Relaxed));
    println!("Alloc pages: {}", ALLOC_PAGES.load(Ordering::Relaxed));
    loop {}
}

mod plic;
mod uart;
mod virtio_hal;
