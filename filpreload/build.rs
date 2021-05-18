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

    cc::Build::new()
        .file("src/_filpreload.c")
        .compile("_filpreload");
    Ok(())
}
