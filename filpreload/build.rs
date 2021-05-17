fn main() -> Result<(), std::io::Error> {
    println!("cargo:rerun-if-changed=src/_filpreload.c");
    println!("cargo:rustc-cdylib-link-arg=-Wl,-export-dynamic");
    // TODO These should be Linux only:
    let cur_dir = std::env::current_dir()?;
    println!(
        "cargo:rustc-cdylib-link-arg=-Wl,--version-script={}/versionscript.txt",
        cur_dir.to_string_lossy()
    );
    println!("cargo:rustc-cdylib-link-arg=-Wl,--defsym=aligned_alloc=reimplemented_aligned_alloc");
    println!("cargo:rustc-cdylib-link-arg=-Wl,--defsym=mmap=fil_mmap_impl");
    println!("cargo:rustc-cdylib-link-arg=-Wl,--defsym=mmap64=fil_mmap_impl");
    cc::Build::new()
        .file("src/_filpreload.c")
        .compile("_filpreload");
    Ok(())
}
