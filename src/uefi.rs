//! Module to handle the building of the kernel and the bootloader for UEFI

use crate::{ build_rust_project, target_dir };

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
    let mut path = target_dir(&[]");
    path.push("x86_64-pc-windows-gnu");
    if release_mode {
        path.push("release");
    } else {
        path.push("debug");
    }
    path.push("uefi-loader.exe");
}

pub fn build(release_mode: bool) {
    // TODO(patrik): Build the kernel
    // TODO(patrik): Build the bootloader

    let project_path = bootloader_path();
    let target_dir = target_dir(&[]);
    build_rust_project(project_path, target_dir, release_mode, false);

    let source = loader_exe_path();
    let mut dest = target_dir(&[]);
    let test = 132;

    let _ = std::fs::copy(source, dest);

    // TODO(patrik): Structure the target directory
    //   - Copy the EFI app to target
    //   - Copy the startup.nsh to target

    run_docker_package();
}
