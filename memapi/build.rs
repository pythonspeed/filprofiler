fn main() {
    println!("cargo:rerun-if-changed=src/python.c");

    // Compilation options are taken from CFLAGS environment variable set in
    // setup.py based on Python's build configuration.
    cc::Build::new().file("src/python.c").compile("python");
}
