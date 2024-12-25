use flate2::read::GzDecoder;
use std::env;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Archive;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn download_slang(download_to: &Path) -> Result<PathBuf> {
    // Keep the version the same as the one in `CMakeLists.txt`
    let target = "https://github.com/MikePopoloski/slang/archive/refs/tags/v7.0.tar.gz";

    fs::create_dir_all(download_to)?;

    // Download source
    let archive_path = download_to.join("slang-linux.tar.gz");
    let mut dest = File::create(&archive_path)?;
    reqwest::blocking::get(target)?.copy_to(&mut dest)?;
    drop(dest);

    // Unpack archive
    let mut archive = Archive::new(GzDecoder::new(File::open(archive_path)?));
    archive.unpack(download_to)?;

    // Return the source directory
    let entries = fs::read_dir(&download_to)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.metadata().is_ok_and(|e| e.is_dir()))
        .collect::<Vec<_>>();
    // Expected exactly one directory in archive
    assert_eq!(entries.len(), 1);
    Ok(entries.first().unwrap().path())
}

fn build_slang(slang_src: &Path, slang_install: &Path) {
    cmake::Config::new(slang_src)
        .profile("Release")
        .define("SLANG_USE_MIMALLOC", "OFF")
        .out_dir(slang_install)
        .build();
}

fn build_slang_wrapper(slang: &Path, wrapper_install: &Path) {
    cmake::Config::new("slang_wrapper")
        .profile("Release")
        .define("CMAKE_PREFIX_PATH", slang)
        .out_dir(wrapper_install)
        .build();
}

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    println!("cargo:rerun-if-changed=slang_wrapper");

    let (slang_install, wrapper_install, link_type) = match env::var("SLANG_INSTALL_PATH") {
        Err(_) => {
            // Build slang from source
            let download_dir = out_dir.join("slang-src");
            let slang_src = download_slang(&download_dir)?;
            let slang_install = out_dir.join("slang-install");
            let wrapper_install = out_dir.join("slang-wrapper-install");

            build_slang(&slang_src, &slang_install);
            build_slang_wrapper(&slang_install, &wrapper_install);

            (
                slang_install.join("lib"),
                wrapper_install.join("lib"),
                "static",
            )
        }
        Ok(slang_install) => {
            // Directly use external slang
            let slang_install = Path::new(&slang_install);
            let wrapper_install = out_dir.join("slang-wrapper-install");

            build_slang_wrapper(slang_install, &wrapper_install);

            (
                slang_install.join("lib"),
                wrapper_install.join("lib"),
                "dylib",
            )
        }
    };

    let bindings = bindgen::Builder::default()
        .clang_arg("-x")
        .clang_arg("c++")
        .header("slang_wrapper/src/slang_wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    println!("cargo:rustc-link-search=native={}", slang_install.display());
    println!(
        "cargo:rustc-link-search=native={}",
        wrapper_install.display()
    );
    // println!("cargo:rustc-link-search=native=/usr/lib");

    println!("cargo:rustc-link-lib=static=slangwrapper");
    println!("cargo:rustc-link-lib={link_type}=svlang");
    println!("cargo:rustc-link-lib=fmt");
    println!("cargo:rustc-link-lib=dylib=stdc++");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    Ok(())
}
