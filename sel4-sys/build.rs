#![feature(fs_try_exists)]

use std::{
    env,
    fs::{self},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use bindgen::Builder;
use duct::*;

fn main() -> Result<()> {
    let out_dir: PathBuf = env::var("OUT_DIR")?.into();
    let build_dir = out_dir.join("build");

    let sel4_config = SeL4Config::get();

    if fs::try_exists(&build_dir)? {
        fs::remove_dir_all(&build_dir)?;
    }

    fs::create_dir(&build_dir)?;

    cmake_config(sel4_config, &build_dir)?;
    ninja_build(&build_dir)?;

    let inc_dirs: Vec<PathBuf> = get_include_dirs(&build_dir);
    generate_bindings(&out_dir, inc_dirs.into_iter())?;

    println!(
        "cargo:BUILD_DIR={}",
        build_dir.to_str().ok_or(anyhow!("Invalid path"))?
    );

    println!(
        "cargo:rustc-link-search=native={}",
        build_dir
            .join("libsel4")
            .to_str()
            .ok_or(anyhow!("Invalid path"))?
    );

    println!("cargo:rustc-link-lib=static=sel4");
    println!("cargo:rerun-if-changed=sel4");

    Ok(())
}

struct SeL4Config {
    debug: bool,
    platform: String,
    mcs: bool,
    dangerous_code_injection: bool,
    devicetree_overlay: Option<PathBuf>,
}

impl SeL4Config {
    fn get() -> Self {
        SeL4Config {
            debug: Self::debug(),
            platform: Self::platform(),
            mcs: Self::mcs(),
            dangerous_code_injection: Self::dangerous_code_injection(),
            devicetree_overlay: Self::devicetree_overlay(),
        }
    }

    fn debug() -> bool {
        match () {
            #[cfg(debug_assertions)]
            () => true,
            #[cfg(not(debug_assertions))]
            () => false,
        }
    }

    fn platform() -> String {
        match () {
            #[cfg(feature = "stm32mp1")]
            () => "stm32mp1".to_string(),
            #[cfg(feature = "zynq7000")]
            () => "zynq7000".to_string(),
        }
    }

    fn mcs() -> bool {
        match () {
            #[cfg(feature = "mcs")]
            () => true,
            #[cfg(not(feature = "mcs"))]
            () => false,
        }
    }

    fn dangerous_code_injection() -> bool {
        match () {
            #[cfg(feature = "dangerous-code-injection")]
            () => true,
            #[cfg(not(feature = "dangerous-code-injection"))]
            () => false,
        }
    }

    fn devicetree_overlay() -> Option<PathBuf> {
        println!("cargo:rerun-if-env-changed=DEVICETREE_OVERLAY");
        env::var("DEVICETREE_OVERLAY").ok().map(String::into)
    }

    fn get_cmake_args(&self) -> Vec<String> {
        vec![
            format!("-DRELEASE={}", if self.debug { "FALSE" } else { "TRUE" }),
            format!("-DPLATFORM={}", &self.platform),
            format!("-DKernelIsMCS={}", if self.mcs { "ON" } else { "OFF" }),
            format!(
                "-DKernelDangerousCodeInjection={}",
                if self.dangerous_code_injection {
                    "ON"
                } else {
                    "OFF"
                }
            ),
            format!(
                "-DKernelCustomDTSOverlay={}",
                if let Some(dts) = self.devicetree_overlay.as_ref() {
                    dts.to_str().unwrap_or("")
                } else {
                    ""
                }
            ),
        ]
    }
}

fn cmake_config(config: SeL4Config, build_dir: impl AsRef<Path>) -> Result<()> {
    let build_dir = build_dir.as_ref();

    let mut args = vec![
        format!("-G {}", "Ninja"),
        format!("-DCROSS_COMPILER_PREFIX={}", "arm-linux-gnueabi-"),
        format!("-DCMAKE_TOOLCHAIN_FILE={}", "kernel/gcc.cmake"),
        format!("-S {}", "sel4"),
        format!("-B {}", build_dir.to_str().unwrap()),
        format!("-DVERIFICATION={}", "FALSE"),
        format!("-DLibSel4FunctionAttributes={}", "public"),
    ];

    args.append(&mut config.get_cmake_args());

    let output = cmd("cmake", args).read()?;

    for line in output.lines() {
        println!("{}", line);
    }

    Ok(())
}

fn ninja_build(build_dir: impl AsRef<Path>) -> Result<()> {
    let build_dir = build_dir.as_ref();

    let output = cmd!("ninja", "-C", build_dir).read()?;

    for line in output.lines() {
        println!("{}", line);
    }

    Ok(())
}

fn get_include_dirs(build_dir: impl AsRef<Path>) -> Vec<PathBuf> {
    let build_dir: PathBuf = build_dir.as_ref().into();
    let sel4_dir: PathBuf = env::current_dir().unwrap().join("sel4");

    let (arch, sel4_arch, sel4_plat, mode) = match () {
        #[cfg(feature = "stm32mp1")]
        () => ("arm", "aarch32", "stm32mp1", "32"),
        #[cfg(feature = "zynq7000")]
        () => ("arm", "aarch32", "zynq7000", "32"),
    };

    vec![
        sel4_dir.join("kernel/libsel4/include"),
        sel4_dir.join(format!("kernel/libsel4/arch_include/{arch}")),
        sel4_dir.join(format!("kernel/libsel4/sel4_arch_include/{sel4_arch}")),
        sel4_dir.join(format!("kernel/libsel4/sel4_plat_include/{sel4_plat}")),
        sel4_dir.join(format!("kernel/libsel4/mode_include/{mode}")),
        build_dir.join("kernel/gen_config"),
        build_dir.join("libsel4/autoconf"),
        build_dir.join("libsel4/gen_config"),
        build_dir.join("libsel4/include"),
        build_dir.join(format!("libsel4/arch_include/{arch}")),
        build_dir.join(format!("libsel4/sel4_arch_include/{sel4_arch}")),
    ]
}

fn generate_bindings(
    out_dir: impl AsRef<Path>,
    include_dirs: impl Iterator<Item = impl AsRef<Path>>,
) -> Result<()> {
    let out_dir = out_dir.as_ref();

    let bindings = Builder::default()
        .header("sel4/kernel/libsel4/include/sel4/sel4.h")
        .clang_arg("--target=arm-linux-gnueabi")
        .clang_args(
            include_dirs
                .into_iter()
                .map(|dir| format!("-I{}", dir.as_ref().to_str().unwrap())),
        )
        .clang_arg("-D KernelDangerousCodeInjection OFF")
        .use_core()
        .layout_tests(false)
        .generate()?;

    bindings.write_to_file(out_dir.join("bindings.rs"))?;

    Ok(())
}
