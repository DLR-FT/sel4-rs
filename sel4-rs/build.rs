use std::{
    env, fs,
    ops::{Range, RangeInclusive},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use indoc::formatdoc;
use quote::quote;

fn main() -> Result<()> {
    let sel4_build_dir: PathBuf = env::var("DEP_SEL4_BUILD_DIR")?.into();
    let out_dir: PathBuf = env::var("OUT_DIR")?.into();

    let mem_regions = read_memory_regions(&sel4_build_dir)?;
    let (kernel_region, kernel_entry) = read_kernel_region(&sel4_build_dir)?;
    let devicetree_region = read_device_tree_range(&kernel_region, &sel4_build_dir)?;

    generate_memory_x(mem_regions, &out_dir)?;
    generate_kernel_rs(&kernel_entry, &kernel_region, &out_dir)?;
    generate_sel4_linker_overlay(&kernel_region, &devicetree_region, &out_dir)?;

    println!(
        "cargo:rustc-link-search={}",
        out_dir.to_str().ok_or(anyhow!("Invalid path"))?
    );

    Ok(())
}

struct MemoryRegion {
    virt_addr: Option<u64>,
    phys_addr: u64,
    len: u64,
}

#[allow(dead_code)]
impl MemoryRegion {
    fn virt_range(&self) -> Option<Range<u64>> {
        self.virt_addr
            .map(|virt_addr| virt_addr..virt_addr + self.len)
    }

    fn virt_range_inclusive(&self) -> Option<RangeInclusive<u64>> {
        self.virt_addr
            .map(|virt_addr| virt_addr..=virt_addr + self.len - 1)
    }

    fn phys_range(&self) -> Range<u64> {
        self.phys_addr..self.phys_addr + self.len
    }

    fn phys_range_inclusive(&self) -> RangeInclusive<u64> {
        self.phys_addr..=self.phys_addr + self.len - 1
    }
}

fn read_memory_regions(sel4_build_dir: impl AsRef<Path>) -> Result<Vec<MemoryRegion>> {
    let sel4_build_dir = sel4_build_dir.as_ref();

    let dt_buf = fs::read(sel4_build_dir.join("kernel").join("kernel.dtb"))?;
    let dt = fdt::Fdt::new(&dt_buf).map_err(|_| anyhow!("Invalid kernel device tree"))?;

    let mem_regions: Vec<_> = dt
        .memory()
        .regions()
        .filter_map(|reg| {
            reg.size.map(|size| {
                let start_addr = reg.starting_address as u64;
                MemoryRegion {
                    virt_addr: None,
                    phys_addr: start_addr,
                    len: size as u64,
                }
            })
        })
        .collect();

    Ok(mem_regions)
}

struct KernelEntry {
    virt_entry: u64,
}

fn read_kernel_region(sel4_build_dir: impl AsRef<Path>) -> Result<(MemoryRegion, KernelEntry)> {
    let sel4_build_dir = sel4_build_dir.as_ref();

    let buf = fs::read(sel4_build_dir.join("kernel").join("kernel.elf"))?;
    let kernel = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(&buf)?;

    let virt_entry = kernel.ehdr.e_entry;

    let segments: Vec<_> = kernel
        .segments()
        .ok_or(anyhow!("No segmentes found in kernel.elf"))?
        .iter()
        .filter(|segment| segment.p_memsz != 0)
        .filter(|segment| segment.p_type == 1)
        .collect();

    if segments.is_empty() {
        return Err(anyhow!("No LOAD segment found in kernel.elf."));
    }

    if segments.len() > 1 {
        return Err(anyhow!(
            "Found {} LOAD segments in kernel.elf; expected 1 LOAD segment.",
            segments.len()
        ));
    }

    //let virt_addr = segments[0].p_vaddr as u64;
    //let virt_range = virt_addr..virt_addr + segments[0].p_memsz as u64;
    //let phys_addr = segments[0].p_paddr as u64;

    Ok((
        MemoryRegion {
            virt_addr: Some(segments[0].p_vaddr),
            phys_addr: segments[0].p_paddr,
            len: segments[0].p_memsz,
        },
        KernelEntry { virt_entry },
    ))
}

fn read_device_tree_range(
    kernel_range: &MemoryRegion,
    sel4_build_dir: impl AsRef<Path>,
) -> Result<MemoryRegion> {
    let sel4_build_dir = sel4_build_dir.as_ref();

    let buf = fs::read(sel4_build_dir.join("kernel").join("kernel.dtb"))?;

    Ok(MemoryRegion {
        virt_addr: Some(kernel_range.phys_range().end),
        phys_addr: kernel_range.phys_range().end,
        len: buf.len() as u64,
    })
}

fn generate_memory_x(
    mem_regions: impl IntoIterator<Item = MemoryRegion>,
    out_dir: impl AsRef<Path>,
) -> Result<()> {
    let out_dir = out_dir.as_ref();

    let mem_lines: Vec<String> = mem_regions
        .into_iter()
        .enumerate()
        .map(|(i, region)| {
            format!(
                "RAM{} (rwx) : ORIGIN = {:#X}, LENGTH = {:#X}",
                match i {
                    0 => "".to_string(),
                    _ => i.to_string(),
                },
                region.phys_addr,
                region.len
            )
        })
        .collect();

    let mem_lines = mem_lines.join("\n");
    let memory_x = formatdoc!(
        r#"MEMORY {{
            {}
        }}"#,
        mem_lines
    );

    fs::write(out_dir.join("memory.x"), memory_x)?;

    Ok(())
}

fn generate_kernel_rs(
    kernel_entry: &KernelEntry,
    kernel_region: &MemoryRegion,
    out_dir: impl AsRef<Path>,
) -> Result<()> {
    let out_dir = out_dir.as_ref();

    let kernel_virt_entry = kernel_entry.virt_entry;
    let kernel_virt_range = kernel_region
        .virt_range_inclusive()
        .ok_or(anyhow!("Kernel virtual region unknown."))?;

    let kernel_virt_start = kernel_virt_range.start();
    let kernel_virt_end = kernel_virt_range.end();
    let kernel_phys_addr = kernel_region.phys_addr;

    let memory_rs = quote! {
        use core::ops::RangeInclusive;
        pub(crate) const KERNEL_VIRT_ENTRY: usize = #kernel_virt_entry as usize;
        pub(crate) const KERNEL_VIRT_RANGE: RangeInclusive<usize> = (#kernel_virt_start as usize)..=(#kernel_virt_end as usize);
        pub(crate) const KERNEL_PHYS_ADDR: usize = #kernel_phys_addr as usize;
    }
    .to_string();

    fs::write(out_dir.join("kernel.rs"), memory_rs)?;

    Ok(())
}

fn generate_sel4_linker_overlay(
    kernel_region: &MemoryRegion,
    devicetree_region: &MemoryRegion,
    out_dir: impl AsRef<Path>,
) -> Result<()> {
    let out_dir = out_dir.as_ref();

    let kernel_ld = formatdoc!(
        r#"SECTIONS {{
            .kernel : {{
                __kernel_start = {:#X};
                . = __kernel_start;
                FILL(0);
                __kernel_end = __kernel_start + {:#X};
                . = __kernel_end;
            }} > RAM

            .devicetree : {{
                __devicetree_start = {:#X};
                . = __devicetree_start;
                FILL(0);
                __devicetree_end = __devicetree_start + {:#X};
                . = __devicetree_end;
            }} > RAM

            .fill : {{
                . = ALIGN(1M);
                __rootserver_start = .;
            }}
        }}
        INSERT BEFORE .text;

        SECTIONS {{
            . = ALIGN(0x1000);
            __rootserver_end = .;
        }}
        INSERT AFTER .stack;
        "#,
        kernel_region.phys_addr,
        kernel_region.len,
        devicetree_region.phys_addr,
        devicetree_region.len
    );

    fs::write(out_dir.join("sel4-overlay.ld"), kernel_ld)?;

    Ok(())
}
