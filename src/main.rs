#![no_std]
#![no_main]
#![feature(allocator_api)]
#![feature(naked_functions)]
#![feature(const_mut_refs)]
#![feature(panic_info_message)]
#![feature(stdsimd)]
#![cfg_attr(debug_assertions, feature(core_intrinsics))]

use alloc::format;
use bar32alloc::PciMemory32Allocator;
use log::*;
use uart::UartLogger;
use virtio_drivers::{
    device::{
        blk::{VirtIOBlk, SECTOR_SIZE},
        console::VirtIOConsole,
    },
    transport::{
        mmio::{MmioTransport, VirtIOHeader},
        pci::{
            bus::{BarInfo, Cam, Command, DeviceFunction, MemoryBarType, PciRoot},
            virtio_device_type, PciTransport,
        },
        Transport,
    },
};

//use talc::*;
//use core::alloc::{Allocator, Layout};
//static mut ARENA: [u8; 10000] = [0; 10000];
//#[global_allocator]
//static ALLOCATOR: Talck<spin::Mutex<()>, InitOnOom> = Talc::new(unsafe {
//    // if we're in a hosted environment, the Rust runtime may allocate before
//    // main() is called, so we need to initialize the arena automatically
//    InitOnOom::new(Span::from_slice(ARENA.as_slice() as *const [u8] as *mut [u8]))
//}).lock();

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
static LOGGER: UartLogger = UartLogger {};
//imports
//basic rust things
extern crate alloc;
use crate::{
    uart::init_from_mmio,
    virtio_hal::{HalImpl, ALLOC_PAGES, OPEN_PAGES},
};
use core::{panic::PanicInfo, ptr::NonNull, sync::atomic::Ordering};

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

fn allocate_bars(
    root: &mut PciRoot,
    device_function: DeviceFunction,
    allocator: &mut PciMemory32Allocator,
) {
    let mut bar_index = 0;
    while bar_index < 6 {
        let info = root.bar_info(device_function, bar_index).unwrap();
        debug!("BAR {}: {}", bar_index, info);
        // Ignore I/O bars, as they aren't required for the VirtIO driver.
        if let BarInfo::Memory {
            address_type, size, ..
        } = info
        {
            match address_type {
                MemoryBarType::Width32 => {
                    if size > 0 {
                        let address = allocator.allocate_memory_32(size);
                        debug!("Allocated address {:#010x}", address);
                        root.set_bar_32(device_function, bar_index, address);
                    }
                }
                MemoryBarType::Width64 => {
                    if size > 0 {
                        let address = allocator.allocate_memory_32(size);
                        debug!("Allocated address {:#010x}", address);
                        root.set_bar_64(device_function, bar_index, address.into());
                    }
                }

                _ => panic!("Memory BAR address type {:?} not supported.", address_type),
            }
        }

        bar_index += 1;
        if info.takes_two_entries() {
            bar_index += 1;
        }
    }

    // Enable the device to use its BARs.
    root.set_command(
        device_function,
        Command::IO_SPACE | Command::MEMORY_SPACE | Command::BUS_MASTER,
    );
    let (status, command) = root.get_status_command(device_function);
    debug!(
        "Allocated BARs and enabled device, status {:?} command {:?}",
        status, command
    );
}

#[allow(dead_code)]
fn write_block<T>(block: &mut VirtIOBlk<HalImpl, T>, data: &[u8], page: usize) -> usize
where
    T: Transport,
{
    for (off, chunk) in data.chunks(SECTOR_SIZE).enumerate() {
        let mut buffer: [u8; SECTOR_SIZE] = [0u8; SECTOR_SIZE];
        let target = page + off;
        #[cfg(debug_assertions)]
        unsafe {
            core::intrinsics::breakpoint()
        }
        let _ = block.read_blocks(target, &mut buffer);
        buffer[..chunk.len()].copy_from_slice(chunk);
        let _ = block.write_blocks(target, &buffer);
    }
    return data.chunks(SECTOR_SIZE).len();
}

/// now we can start cooking, our real code exist here
extern "C" fn entry(_hard_id: u64, fdt_ptr: *const u8) -> ! {
    unsafe {
        let _ = log::set_logger(&LOGGER);
        if true {
            log::set_max_level(log::LevelFilter::Trace);
        } else {
            log::set_max_level(log::LevelFilter::Warn);
        }
        //init the virtio hal as `lazy_static` doesen't exist
        virtio_hal::init_virtio_hal();

        //setup the globals
        DEVICE_TREE_PTR = fdt_ptr; //device tree ptr
        let dev_tree = fdt::Fdt::from_ptr(fdt_ptr).expect("fdt pointer no exist?");
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
        let pci_node = dev_tree
            .find_compatible(&["pci-host-ecam-generic"])
            .unwrap();
        let pci_addr = pci_node.reg().unwrap().next().unwrap().starting_address;
        let mut pci = PciRoot::new(pci_addr as *mut u8, Cam::Ecam);
        let mut allocator = PciMemory32Allocator::for_pci_ranges(&pci_node);
        #[allow(unused_mut,unused_variables)]
        let mut console: Option<VirtIOConsole<HalImpl, PciTransport>> = {
            let mut ret = None;
            for a in 0..255 {
                for (i, j) in pci.enumerate_bus(a) {
                    println!("pci device: {:#X}:{:#X}", j.vendor_id, j.device_id);
                    if let Some(dev_type) = virtio_device_type(&j) {
                        println!("devtype: {:?}", dev_type);
                        pci.set_command(
                            i,
                            Command::IO_SPACE | Command::MEMORY_SPACE | Command::BUS_MASTER,
                        );
                        allocate_bars(&mut pci, i, &mut allocator);
                        println!("{:?}", pci.bar_info(i, 4));
                        let console = PciTransport::new::<HalImpl>(&mut pci, i);
                        match console {
                            Ok(pcit) => {
                                println!("virtio_type {:?}", pcit.device_type());
                                ret = Some(VirtIOConsole::new(pcit).unwrap());
                                break;
                            }
                            Err(e) => {
                                println!("error {:?}", e);
                            }
                        }
                    }
                }
            }
            ret
        };

        //if let Some(mut con) = console {
        //    let mut last = None;
        //    while last.is_none() {
        //        if let Some(ch) = con.recv(true).unwrap() {
        //            last = Some(ch as char);
        //        }
        //    }
        //    let mut buffer = vec![last.unwrap()];
        //    while last.is_some() {
        //        match con.recv(true).unwrap() {
        //            Some(c) => buffer.push(c as char),
        //            None => last = None,
        //        }
        //    }
        //    println!("{:?}", buffer);
        //}

        for virt in dev_tree.all_nodes().filter(|node| {
            if let Some(compat) = node.compatible() {
                compat.all().filter(|i| i == &"virtio,mmio").count() > 0
            } else {
                false
            }
        }) {
            //print virtio devices + device IDs
            let device = virt
                .reg()
                .unwrap()
                .next()
                .unwrap()
                .starting_address
                .offset(8) as *const u32;
            println!("virt: {} d:{}", virt.name,*device);
            if *device == 2 {
                println!("device type confirmed");
                //we got ourselfes a storage device
                let header = NonNull::new(
                    virt.reg().unwrap().next().unwrap().starting_address as *mut VirtIOHeader,
                )
                .unwrap();
                println!("got header");
                let transport = MmioTransport::new(header).unwrap();
                println!("transport created");
                let mut ublk = VirtIOBlk::<HalImpl, _>::new(transport).unwrap();
                println!("connected to block");
                let blcks = ublk.capacity();
                println!("Blocks: {}", blcks);
                println!("Size: {} bytes", (blcks * (SECTOR_SIZE as u64)));
                println!("Read Only: {}", ublk.readonly());
                #[cfg(not(debug_assertions))]
                let bstr = b"Hello World!!!";
                #[cfg(debug_assertions)]
                let bstr = b"DEBUG World!!!";
                let mut buf = [0u8; SECTOR_SIZE];
                buf[..bstr.len()].copy_from_slice(bstr.as_slice());
                let _ = ublk.write_blocks(0, &buf);
            } else if *device == 19 {
                println!("HERES OUR SERIAL")
            } else if let Ok(miot) =
                MmioTransport::new(NonNull::new(device as *mut VirtIOHeader).unwrap())
            {
                println!("other device type {:?}", miot.device_type());
            }
        }
        print!("gimme string>");
        let str = readln!();
        println!("here you go: {}", str);
        println!("goodbye");

        panic!("reached end of program")
    }
}

#[cfg(debug_assertions)]
#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    let file = location.file();
    let line = location.line();
    let tmp = format_args!("");
    let error = format!("{}", info.message().unwrap_or(&tmp));
    let err_debug = error.as_str();
    //break-here
    print!("{}@", file); //panic only the file and line
    print!("{}: ", line);
    println!("{}", err_debug);
    println!("DMA pages: {:064b}", OPEN_PAGES.load(Ordering::Relaxed));
    println!("Alloc pages: {}", ALLOC_PAGES.load(Ordering::Relaxed));
    loop {
        //core::hint::spin_loop()
    }
}

#[cfg(not(debug_assertions))]
#[panic_handler]
fn on_panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    println!("DMA pages: {:064b}", OPEN_PAGES.load(Ordering::Relaxed));
    println!("Alloc pages: {}", ALLOC_PAGES.load(Ordering::Relaxed));
    //break-here
    loop {
        //core::hint::spin_loop()
    }
}

mod bar32alloc;
mod plic;
mod stolen_uart;
mod uart;
mod virtio_hal;
