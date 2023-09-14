#![no_std]
#![no_main]
#![feature(naked_functions)]

use core::panic::PanicInfo;

use sel4_rs::{rootserver, Rootserver};

rootserver!(System);
struct System;

impl Rootserver for System {
    fn rootserver() -> ! {
        todo!()
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
