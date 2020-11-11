use flate2::read::GzDecoder;
use std::env;
use std::fs::File;
use std::path::PathBuf;
use tar::Archive;

fn download_slang() -> Result<(), Box<dyn std::error::Error>> {
    let target = "https://github.com/MikePopoloski/slang/releases/download/v0.5/slang-linux.tar.gz";
    let fname = "slang-linux.tar.gz";
    let mut response = reqwest::blocking::get(target)?;
    let mut dest = File::create(fname)?;
    response.copy_to(&mut dest)?;
    let tar = GzDecoder::new(File::open(fname)?);
    let mut archive = Archive::new(tar);
    archive.unpack("slang_wrapper/.")?;
    Ok(())
}

fn build_slang_wrapper() {
    cc::Build::new()
        .cpp(true)
        .flag("-std=c++17")
        .flag("-Wno-type-limits")
        .static_flag(true)
        .include("slang_wrapper/slang/include")
        .file("slang_wrapper/src/slang_lib.cpp")
        .file("slang_wrapper/src/basic_client.cpp")
        .out_dir("slang_wrapper/slang/lib")
        .compile("slangwrapper");
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:rerun-if-changed=slang_wrapper/src/slang_wrapper.h");
    println!("cargo:rerun-if-changed=slang_wrapper/src/slang_lib.cpp");

    download_slang().unwrap();

    build_slang_wrapper();

    let bindings = bindgen::Builder::default()
        .clang_arg("-x")
        .clang_arg("c++")
        .header("slang_wrapper/src/slang_wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    println!(
        "cargo:rustc-link-search=native={}/slang_wrapper/slang/lib",
        env::var("CARGO_MANIFEST_DIR").unwrap()
    );
    // println!("cargo:rustc-link-search=native=/usr/lib");

    println!("cargo:rustc-link-lib=static=slangwrapper");
    println!("cargo:rustc-link-lib=static=slangcompiler");
    println!("cargo:rustc-link-lib=static=slangruntime");
    println!("cargo:rustc-link-lib=static=slangparser");
    println!("cargo:rustc-link-lib=static=slangcore");
    println!("cargo:rustc-link-lib=dylib=stdc++");

    let out_path = PathBuf::from(out_dir);
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
