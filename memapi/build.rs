use std::process::Command;

/// Get paths for C compilation builds, e.g. "include" or "platinclude".
/// TODO this is copy/pasted multiple times...
fn get_python_path(pathname: &str) -> String {
    let output = Command::new("python")
        .arg("-c")
        .arg(format!(
            "import sysconfig; print(sysconfig.get_path('{}'))",
            pathname
        ))
        .output()
        .unwrap();
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn main() {
    println!("cargo:rerun-if-changed=src/python.c");

    // Compilation options are taken from CFLAGS environment variable set in
    // setup.py based on Python's build configuration.
    cc::Build::new()
        .file("src/python.c")
        .include(get_python_path("include"))
        .include(get_python_path("platinclude"))
        .define("_GNU_SOURCE", "1")
        .define("NDEBUG", "1")
        .flag("-fno-omit-frame-pointer")
        .compile("python");
}
