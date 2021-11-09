//! This is the build system for the Rest-OS
//! So this is built and run on the host machine

// TODO(patrik):
//   - Add checks
//     - For rust target is installed
//       - For GRUB we need x86_64-elf
//   - Restructure the program
//   - Add command line options
//     - Debug/Release build
//   - Diffrent building environments
//     - GRUB
//     - UEFI
//       - Compile the UEFI-specific loader

use std::process::Command;
use std::path::{ Path, PathBuf };

use clap::Parser;

mod grub;
mod uefi;

fn linker() -> String {
    let cross = match std::env::var("CROSS") {
        Ok(c) => c,
        Err(_) => "".to_string(),
    };

    let linker = format!("{}ld", cross);

    linker
}

fn target_dir(components: &[&str]) -> PathBuf {
    let mut result = PathBuf::new();
    result.push("target");

    for comp in components {
        result.push(&comp);
    }

    result
}

fn kernel_source(components: &[&str]) -> PathBuf {
    let mut result = PathBuf::new();
    result.push("kernel");
    result.push("src");

    for comp in components {
        result.push(&comp);
    }

    result
}

fn compile_asm<P: AsRef<Path>>(source: P) {
    let source = source.as_ref();

    // Get the filename without the extention
    let file_name = source.file_stem()
        .expect("Failed to retrive the file stem");
    let file_name = file_name.to_str()
        .expect("Failed to convert the file stem to str");
    // Create an string so we can append an extention
    let mut file_name = String::from(file_name);
    // Append an ".o" extention to the target filename
    file_name.push_str(".o");
    let file_name = file_name.as_str();

    // Create the target path
    let target = target_dir(&[file_name]);

    println!("Assembly: {:?} -> {:?}", source, target);

    // Compile the assembly file
    let output = Command::new("nasm")
        .arg("-g")
        .arg("-f")
        .arg("elf64")
        .arg(source)
        .arg("-o")
        .arg(target)
        .output()
            .expect("Unknown error when running 'nasm' (is nasm installed?)");

    if !output.status.success() {
        let error_message = std::str::from_utf8(&output.stderr)
            .expect("Failed to convert stderr to str");
        eprintln!("Error Message:\n{}", error_message);

        std::process::exit(-1);
    }
}

fn build_rust_project<P: AsRef<Path>>(project_path: P, target_path: P,
                                      release_mode: bool,
                                      need_linker: bool)
{
    let project_path = project_path.as_ref();
    let target_path = target_path.as_ref().canonicalize()
        .expect("Failed to cononicalize the target path");
    println!("Building rust: {:?} -> {:?}", project_path, target_path);

    let linker = linker();

    let mut command = Command::new("cargo");
    command.current_dir(project_path);
    if need_linker {
        command.env("RUSTFLAGS", format!("-C linker={}", linker));
    }

    command.arg("build");

    if release_mode {
        command.arg("--release");
    }

    command.arg("--target-dir");
    command.arg(target_path);

    let status = command.status()
        .expect("Unknown error when running 'cargo'");

    if !status.success() {
        std::process::exit(-1);
    }
}

fn link_executable<P>(obj_file: P, target: P, linker_script: P)
    where P: AsRef<Path>
{
    let target = target.as_ref();
    let linker_script = linker_script.as_ref();
    let obj_file = obj_file.as_ref();

    let linker = linker();

    let output = Command::new(linker)
        .arg("-n")
        .arg("-T")
        .arg(&linker_script)
        .arg("-o")
        .arg(target)
        .arg(obj_file)
        .output()
            .expect("Unknown error when running 'ld' (is ld installed?)");

        //.arg("target/x86_64-rest-os/debug/librest_os.a");

    if !output.status.success() {
        let error_message = std::str::from_utf8(&output.stderr).ok().unwrap();
        eprintln!("Linking Error Message:\n{}", error_message);

        std::process::exit(-1);
    }
}

fn build_userland_bin(name: &str) {
    let mut project_path = PathBuf::new();
    project_path.push("userland");
    project_path.push(name);

    let mut target_path = PathBuf::new();
    target_path.push("target");
    target_path.push("userland");
    target_path.push(name);

    println!("Project Path: {:?}", project_path);
    println!("Target Path: {:?}", target_path);

    let _ = std::fs::create_dir(&target_path);
    build_rust_project(project_path, target_path, false, true);
}

fn copy_userland_bin_to_initrd(name: &str) {
    // target/userland/init/x86_64/debug/init
    let mut source = PathBuf::new();
    source.push("target");
    source.push("userland");
    source.push(name);
    source.push("x86_64-rest-os");
    source.push("debug");
    source.push(name);

    let mut dest = PathBuf::new();
    dest.push("target");
    dest.push("initrd");
    dest.push(name);

    let _ = std::fs::copy(source, dest);
}

fn build_initrd() {
    let status = Command::new("./build_initrd.sh")
        .current_dir("misc")
        .status()
            .expect("Failed to run 'build_initrd.sh'");

    if !status.success() {
        eprintln!("Failed to run './build_initrd.sh'");

        std::process::exit(-1);
    }
}

fn prepare_initrd() {
    build_userland_bin("init");
    copy_userland_bin_to_initrd("init");

    build_initrd();
}

#[derive(Parser, Debug)]
#[clap(version = "0.0.1", author = "Patrik M. Rosenström <patrik.millvik@gmail.com>")]
struct Opts {
    #[clap(short, long)]
    release: bool,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Parser, Debug)]
enum Commands {
    BuildGrub(BuildGrub),
    BuildUefi(BuildUefi),
}

#[derive(Parser, Debug)]
struct BuildGrub {}

#[derive(Parser, Debug)]
struct BuildUefi {}

fn main() {
    // Parse the command line arguments
    let opts = Opts::parse();

    // Create target the directories
    let _ = std::fs::create_dir("target");
    let _ = std::fs::create_dir("target/userland");
    let _ = std::fs::create_dir("target/initrd");

    match opts.command {
        Commands::BuildGrub(_) => {
            grub::build(opts.release);
        }

        Commands::BuildUefi(_) => {
            uefi::build(opts.release);
        }
    }
}
