#![no_std]
#![feature(naked_functions)]

use core::{arch::asm, marker::PhantomData, mem::transmute};

#[cfg(feature = "zynq7000")]
pub use zynq7000 as platform;

use platform::arch::{self, NORMAL};

mod kernel;

#[macro_export]
macro_rules! rootserver {
    ($RootserverImpl:ty) => {
        use sel4_rs;
        use sel4_rs::platform::arch;

        arch::startup!(sel4_rs::System<$RootserverImpl>);
    };
}

pub trait Rootserver {
    fn rootserver() -> !;
}

pub struct System<R: Rootserver>(PhantomData<R>);

impl<R: Rootserver> arch::MemoryMap for System<R> {
    const MAP: &'static [arch::MemoryRegion] = &[arch::MemoryRegion::sections(
        0..=*kernel::KERNEL_VIRT_RANGE.start() - 1,
        0,
        NORMAL.read_writeable().executeable(),
    )];
}

impl<R: Rootserver> arch::EntryPoint for System<R> {
    fn main() -> ! {
        let kernel_entry = kernel::KERNEL_VIRT_ENTRY as *const ();
        let kernel: extern "C" fn(usize, usize, usize, usize, usize, usize) -> ! =
            unsafe { transmute(kernel_entry) };

        extern "C" {
            static mut __kernel_start: u8;
            static mut __kernel_end: u8;
            static mut __devicetree_start: u8;
            static mut __devicetree_end: u8;
            static mut __rootserver_start: u8;
            static mut __rootserver_end: u8;
        }

        //let kernel_start = unsafe { &__kernel_start as *const u8 as usize };
        //let kernel_end = unsafe { &__kernel_end as *const u8 as usize };
        let devicetree_start = unsafe { &__devicetree_start as *const u8 as usize };
        let devicetree_end = unsafe { &__devicetree_end as *const u8 as usize };
        let rootserver_start = unsafe { &__rootserver_start as *const u8 as usize };
        let rootserver_end = unsafe { &__rootserver_end as *const u8 as usize };

        // arch::MemoryRegion::sections(
        //     0..=*kernel::KERNEL_VIRT_RANGE.start() - 1,
        //     kernel::KERNEL_PHYS_ADDR,
        //     NORMAL.read_writeable().executeable(),
        // )
        // .map();

        arch::MemoryRegion::sections(
            (*kernel::KERNEL_VIRT_RANGE.start())..=(0xFFFF_FFFF),
            kernel::KERNEL_PHYS_ADDR,
            NORMAL.read_writeable().executeable(),
        )
        .map();

        kernel(
            rootserver_start,
            rootserver_end,
            0,
            rootserver_entry::<R> as *const fn() as usize,
            devicetree_start,
            devicetree_end - devicetree_start,
        );
    }
}

#[naked]
//#[no_mangle]
pub(crate) unsafe extern "C" fn rootserver_entry<R: Rootserver>() -> ! {
    asm!(
        "ldr sp, =__sys_stack_end",
        "bl {rootserver}",
        rootserver = sym R::rootserver,
        options(noreturn)
    );
}
