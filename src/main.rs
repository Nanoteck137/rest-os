//! This is the build system for the Rest-OS
//! So this is built and run on the host machine

use std::process::Command;

use std::path::{ Path, PathBuf };

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

fn compile_asm<P: AsRef<Path>>(source: P) -> Option<()> {
    let source = source.as_ref();

    // Get the filename without the extention
    let file_name = source.file_stem()?.to_str()?;
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
        let error_message = std::str::from_utf8(&output.stderr).ok()?;
        eprintln!("Error Message:\n{}", error_message);
        return None;
    }

    Some(())
}

// Builds all the Assembly files needed for the kernel
fn build_kernel_asm_files() -> Option<()> {
    // Boot.asm

    let asm_source =
        kernel_source(&["arch", "x86_64", "boot", "boot.asm"]);
    compile_asm(asm_source)?;

    Some(())
}

fn build_rust_project<P: AsRef<Path>>(project_path: P, target_path: P)
    -> Option<()>
{
    let project_path = project_path.as_ref();
    let target_path = target_path.as_ref().canonicalize().ok()?;
    println!("Building rust: {:?} -> {:?}", project_path, target_path);
    let status = Command::new("cargo")
        .current_dir(project_path)
        .arg("build")
        .arg("--target-dir")
        .arg(target_path)
        .status()
            .expect("Unknown error when running 'cargo'");

    if !status.success() {
        return None;
    }

    Some(())
}

fn build_rust_projects() -> Option<()> {
    build_rust_project("kernel", "target")?;

    Some(())
}

fn build_userland_bin(name: &str) -> Option<()> {
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
    build_rust_project(project_path, target_path)?;

    Some(())
}

fn copy_userland_bin_to_initrd(name: &str) -> Option<()> {
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

    Some(())
}

fn build_initrd() -> Option<()> {
    let status = Command::new("./build_initrd.sh")
        .current_dir("misc")
        .status()
            .expect("Failed to run 'build_initrd.sh'");

    if !status.success() {
        return None;
    }

    Some(())
}

fn prepare_initrd() -> Option<()> {
    build_userland_bin("init")?;
    copy_userland_bin_to_initrd("init")?;

    build_initrd()?;

    Some(())
}

fn main() {
    println!("Building Rest-OS");

    let _ = std::fs::create_dir("target");
    let _ = std::fs::create_dir("target/userland");
    let _ = std::fs::create_dir("target/initrd");
    let _ = std::fs::create_dir("target/isofiles");
    let _ = std::fs::create_dir("target/isofiles/boot");
    let _ = std::fs::create_dir("target/isofiles/boot/grub");

    println!("Target directory: {:?}", target_dir(&[]));
    println!("Kernel Source directory: {:?}", kernel_source(&[]));

    build_kernel_asm_files().expect("Failed to build the assembly files");
    build_rust_projects().expect("Failed to build the rust projects");

    let target = target_dir(&["kernel.elf"]);
    let output = Command::new("x86_64-elf-ld")
        .arg("-n")
        .arg("-T")
        .arg("kernel/src/arch/x86_64/linker.ld")
        .arg("-o")
        .arg(target)
        .arg("target/x86_64-rest-os/debug/librest_os.a")
        .output()
            .expect("Unknown error when running 'ld' (is ld installed?)");

    if !output.status.success() {
        let error_message = std::str::from_utf8(&output.stderr).ok().unwrap();
        eprintln!("Linking Error Message:\n{}", error_message);
    }

    let source = target_dir(&["kernel.elf"]);
    let dest = target_dir(&["isofiles", "boot", "kernel"]);

    let _ = std::fs::copy(source, dest);

    let source = "misc/grub.cfg";
    let dest = target_dir(&["isofiles", "boot", "grub", "grub.cfg"]);

    let _ = std::fs::copy(source, dest);

    println!("Preparing initrd");
    prepare_initrd()
        .expect("Failed to prepare initrd");

    let source = target_dir(&["initrd.cpio"]);
    let dest = target_dir(&["isofiles", "boot", "initrd.cpio"]);

    let _ = std::fs::copy(source, dest);

    println!("Creating the Image");

    let target = target_dir(&["image.iso"]);
    let iso_dir = target_dir(&["isofiles"]);
    let output = Command::new("grub-mkrescue")
        .arg("-o")
        .arg(target)
        .arg(iso_dir)
        .output()
            .expect("Unknown error when running 'grub-mkrescue' (is grub-mkrescue installed?)");

    if !output.status.success() {
        let error_message = std::str::from_utf8(&output.stderr).ok().unwrap();
        eprintln!("Error Message:\n{}", error_message);
    }

}

