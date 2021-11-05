use std::process::Command;

/// Get paths for C compilation builds, e.g. "include" or "platinclude".
/// TODO this is copy/pasted multiple times...
fn get_python_path(pathname: &str) -> String {
    let exe = std::env::var("PYO3_PYTHON").unwrap_or("python".to_string());
    let output = Command::new(exe)
        .arg("-c")
        .arg(format!(
            "import sysconfig; print(sysconfig.get_path('{}'))",
            pathname
        ))
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn main() -> Result<(), std::io::Error> {
    println!("cargo:rerun-if-changed=src/_filpreload.c");
    let cur_dir = std::env::current_dir()?;

    #[cfg(target_os = "linux")]
    {
        // On Linux GNU ld can't handle two version files (one from Rust, one from
        // us) at the same time without blowing up.
        println!("cargo:rustc-cdylib-link-arg=-fuse-ld=gold");

        // Use a versionscript to limit symbol visibility.
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}/versionscript.txt",
            cur_dir.to_string_lossy()
        );
        // Make sure aligned_alloc() is public under its real name; workaround for
        // old glibc headers in Conda.
        println!(
            "cargo:rustc-cdylib-link-arg=-Wl,--defsym=aligned_alloc=reimplemented_aligned_alloc"
        );
        // On 64-bit Linux, mmap() is another way of saying mmap64, or vice versa,
        // so we point to function of our own.
        println!("cargo:rustc-cdylib-link-arg=-Wl,--defsym=mmap=fil_mmap_impl");
        println!("cargo:rustc-cdylib-link-arg=-Wl,--defsym=mmap64=fil_mmap_impl");
    };

    // Compilation options are taken from Python's build configuration.
    cc::Build::new()
        .file("src/_filpreload.c")
        .include(get_python_path("include"))
        .include(get_python_path("platinclude"))
        .define("_GNU_SOURCE", "1")
        .define("NDEBUG", "1")
        .flag("-fno-omit-frame-pointer")
        .compile("_filpreload");
    Ok(())
}
