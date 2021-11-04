//! Module to handle the building of the kernel for the grub bootloader

use crate::{ kernel_source, compile_asm, build_rust_project };
use crate::{ link_executable, target_dir, prepare_initrd };

use std::path::{ Path, PathBuf };
use std::process::Command;

/// Creates a path to the boot.asm out GRUB loader uses to bootstrap the OS
fn boot_asm_path() -> PathBuf {
    let mut kernel_source = kernel_source(&[]);
    kernel_source.push("arch");
    kernel_source.push("x86_64");
    kernel_source.push("boot");
    kernel_source.push("boot.asm");

    kernel_source
}

/// Creates a path to the kernel archive
fn kernel_archive(release_mode: bool) -> PathBuf {
    let mut path = target_dir(&[]);
    path.push("x86_64-rest-os");

    if release_mode {
        path.push("release");
    } else {
        path.push("debug");
    }

    path.push("librest_os.a");

    path
}

/// Creates a path to the kernel executable
/// TODO(patrik): Move to common
fn kernel_executable_target() -> PathBuf {
    let mut path = target_dir(&[]);
    path.push("kernel.elf");

    path
}

/// Creates a path to the linker script our GRUB loader uses
fn linker_script_path() -> PathBuf {
    let mut path = kernel_source(&[]);
    path.push("arch");
    path.push("x86_64");
    path.push("linker.ld");

    path
}

/// Creates a path to the isofiles/boot
fn iso_boot() -> PathBuf {
    let mut path = target_dir(&[]);
    path.push("isofiles");
    path.push("boot");

    path
}

/// Creates a path to the isofiles/boot/grub
fn iso_boot_grub() -> PathBuf {
    let mut path = target_dir(&[]);
    path.push("isofiles");
    path.push("boot");
    path.push("grub");

    path
}

/// Copies files to the boot directroy inside the image
fn copy_to_boot<P>(source: P, target_name: &str)
    where P: AsRef<Path>
{
    let mut dest = iso_boot();
    dest.push(target_name);

    let _ = std::fs::copy(source, dest);
}

/// Copies files to the grub directory inside the image
fn copy_to_grub<P>(source: P, target_name: &str)
    where P: AsRef<Path>
{
    let mut dest = iso_boot_grub();
    dest.push(target_name);

    let _ = std::fs::copy(source, dest);
}

/// Creates the path to the grub config file
fn grub_config() -> PathBuf {
    let mut path = PathBuf::new();
    path.push("misc");
    path.push("grub.cfg");

    path
}

/// Creates the final ISO image used for running the OS
fn create_image() {
    // Copy over the kernel executable
    let kernel_exe = kernel_executable_target();
    copy_to_boot(kernel_exe, "kernel");

    // Copy over the initrd archive
    let initrd = target_dir(&["initrd.cpio"]);
    copy_to_boot(initrd, "initrd.cpio");

    // Copy over the grub config
    let grub_config = grub_config();
    copy_to_grub(grub_config, "grub.cfg");

    // Execute 'grub-mkrescue' to make a ISO image
    let target = target_dir(&["image.iso"]);
    let iso_dir = target_dir(&["isofiles"]);
    let output = Command::new("grub-mkrescue")
        .arg("-o")
        .arg(target)
        .arg(iso_dir)
        .output()
            .expect("Unknown error when running 'grub-mkrescue' \
                    (is grub-mkrescue installed?)");

    // Check if 'grub-mkrescue' ran successfully
    if !output.status.success() {
        // Print the stderr stream to we can see the errors
        let error_message = std::str::from_utf8(&output.stderr).ok().unwrap();
        eprintln!("Error Message:\n{}", error_message);
    }
}

pub fn build(release_mode: bool) {
    println!("Building the kernel for GRUB in {} mode",
             if release_mode { "Release" } else { "Debug" });

    // Create the target directories for GRUB
    let _ = std::fs::create_dir("target/isofiles");
    let _ = std::fs::create_dir("target/isofiles/boot");
    let _ = std::fs::create_dir("target/isofiles/boot/grub");

    // Compile the boot.asm needed to bootstrap the OS
    let boot_asm_path = boot_asm_path();
    compile_asm(boot_asm_path);

    // Build the kernel rust project
    build_rust_project("kernel", "target", release_mode, true);

    // Link the kernel executable
    let kernel_archive = kernel_archive(release_mode);
    let kernel_target = kernel_executable_target();
    let kernel_linker_script = linker_script_path();
    link_executable(kernel_archive, kernel_target, kernel_linker_script);

    // Prepare the initrd
    prepare_initrd();

    // Create the final image
    create_image();
}
