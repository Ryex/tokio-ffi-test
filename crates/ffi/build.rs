use cxx_build::CFG;

fn main() {
    // CFG.change_detection = true;
    CFG.doxygen = true;

    cxx_build::bridges(&["src/ffi.rs"]) // returns a cc::Build
        .include("include/")
        .file("src/ffi.cc")
        .file("src/functor.cc")
        .file("src/lib.cc")
        .cpp(true)
        .std("c++17")
        .flag_if_supported("/Zc:__cplusplus")
        .compile("cxx_enum_test");

    println!("cargo:rerun-if-changed=src/ffi.h");
    println!("cargo:rerun-if-changed=src/ffi.cc");
    println!("cargo:rerun-if-changed=src/functor.cc");
    println!("cargo:rerun-if-changed=src/functor.h");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/lib.cc");
    println!("cargo:rerun-if-changed=src/sync.h");
}
