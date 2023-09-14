#![no_std]

use core::marker::PhantomData;

#[cfg(feature = "zynq7000")]
pub use zynq7000 as platform;

use platform::arch;

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
    const MAP: &'static [arch::MemoryRegion] = &[arch::MemoryRegion::image(
        arch::NORMAL.read_writeable().executeable(),
    )];
}

impl<R: Rootserver> arch::EntryPoint for System<R> {
    fn main() -> ! {
        R::rootserver();
    }
}
