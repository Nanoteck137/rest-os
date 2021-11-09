//! Module to handle the building of the kernel and the bootloader for UEFI

use crate::{ build_rust_project, target_dir, link_executable, kernel_source };

use std::process::Command;
use std::path::PathBuf;

fn run_docker_package() {
    // TODO(patrik): Change this
    let target_dir = target_dir(&[]);
    let target_dir = target_dir.canonicalize().unwrap();
    let target_dir = target_dir.as_os_str().to_str().unwrap();

    let mount = format!("{}:/data", target_dir);

    let output = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(mount)
        .arg("rest-os/uefi-image")
        .arg("bash")
        .arg("/build_image.sh")
        .output()
            .expect("Unknown error when running 'docker' \
                    (is docker installed?)");

    if !output.status.success() {
        let error_message = std::str::from_utf8(&output.stderr).ok().unwrap();
        eprintln!("Error Message:\n{}", error_message);
    }
}

fn bootloader_path() -> PathBuf {
    let mut path = PathBuf::new();
    path.push("uefi-loader");

    path
}

fn loader_exe_path(release_mode: bool) -> PathBuf {
    let mut path = target_dir(&[]);
    path.push("x86_64-pc-windows-gnu");
    if release_mode {
        path.push("release");
    } else {
        path.push("debug");
    }
    path.push("uefi-loader.exe");

    path
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
    path.push("uefi-linker.ld");

    path
}

pub fn build(release_mode: bool) {
    // TODO(patrik): Build the kernel
    // TODO(patrik): Build the bootloader

    // Build the kernel rust project
    build_rust_project("kernel", "target", release_mode, true);

    // Link the kernel executable
    let kernel_archive = kernel_archive(release_mode);
    let kernel_target = kernel_executable_target();
    let kernel_linker_script = linker_script_path();
    link_executable(kernel_archive, kernel_target, kernel_linker_script);

    {
        let project_path = bootloader_path();
        let target_dir = target_dir(&[]);
        build_rust_project(project_path, target_dir, release_mode, false);
    }

    let source = loader_exe_path(release_mode);
    let mut dest = target_dir(&[]);
    dest.push("boot.efi");

    let _ = std::fs::copy(source, dest);

    let source = "misc/startup.nsh";
    let mut dest = target_dir(&[]);
    dest.push("startup.nsh");

    let _ = std::fs::copy(source, dest);

    // TODO(patrik): Structure the target directory
    //   - Copy the EFI app to target
    //   - Copy the startup.nsh to target

    run_docker_package();
}
