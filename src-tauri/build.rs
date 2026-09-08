//! Build script: compiles the vendored whisper.cpp (CMake) into static libs,
//! compiles the C shim (cc), and links everything into the Rust build.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    println!("cargo:rustc-env=BL_BUILD_SECS={}", secs);
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=shim/bl_shim.c");
    println!("cargo:rerun-if-changed=whisper-cpp/include");
    println!("cargo:rerun-if-changed=whisper-cpp/src");
    println!("cargo:rerun-if-changed=whisper-cpp/ggml");

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let whisper_dir = manifest_dir.join("whisper-cpp");
    let build_dir = manifest_dir.join("whisper-build");

    if !whisper_dir.exists() {
        panic!("vendored whisper-cpp directory missing");
    }

    build_whisper(&whisper_dir, &build_dir);

    // Static libs may be spread across several CMake output dirs (whisper in
    // src/, ggml in ggml/src/); collect and link every whisper/ggml lib.
    let mut lib_dirs: Vec<PathBuf> = Vec::new();
    collect_lib_dirs(&build_dir, &mut lib_dirs, 0);
    lib_dirs.sort();
    lib_dirs.dedup();

    let mut linked: Vec<String> = Vec::new();
    for dir in &lib_dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap().to_string_lossy().to_lowercase();
            if !(name.ends_with(".lib") || name.ends_with(".a")) {
                continue;
            }
            if !(name.starts_with("whisper") || name.starts_with("ggml")) {
                continue;
            }
            let stem = path
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .trim_start_matches("lib")
                .to_string();
            if linked.contains(&stem) {
                continue;
            }
            println!("cargo:rustc-link-search=native={}", dir.display());
            println!("cargo:rustc-link-lib=static={stem}");
            linked.push(stem);
        }
    }
    if linked.is_empty() {
        panic!("no whisper/ggml static libs found under {}", build_dir.display());
    }

    // Compile the shim against the vendored headers.
    let mut build = cc::Build::new();
    build
        .file("shim/bl_shim.c")
        .include(whisper_dir.join("include"))
        .include(whisper_dir.join("ggml").join("include"))
        .warnings(false);
    build.compile("bl_shim");

    // Standard Tauri build: manifest (comctl32 v6), resources, icons.
    tauri_build::build();
}

fn build_whisper(whisper_dir: &Path, build_dir: &Path) {
    // .cargo/config.toml pins CMAKE to this machine's VS-bundled cmake; if
    // that path does not exist here (different machine / VS layout), fall
    // back to whatever cmake is on PATH.
    let cmake = std::env::var("CMAKE")
        .ok()
        .filter(|p| Path::new(p).exists())
        .unwrap_or_else(|| "cmake".to_string());

    let configured = build_dir.join("CMakeCache.txt").exists();
    if !configured {
        let status = Command::new(&cmake)
            .arg("-S")
            .arg(whisper_dir)
            .arg("-B")
            .arg(build_dir)
            .args([
                "-DBUILD_SHARED_LIBS=OFF",
                "-DWHISPER_BUILD_EXAMPLES=OFF",
                "-DWHISPER_BUILD_TESTS=OFF",
                "-DWHISPER_BUILD_SERVER=OFF",
                "-DGGML_BUILD_EXAMPLES=OFF",
                "-DGGML_BUILD_TESTS=OFF",
                "-DGGML_OPENMP=OFF",
                // Static CRT: keeps whisper/ggml from importing newer
                // MSVCP140 exports than the host machine's runtime provides.
                // Set both the modern variable AND explicit flags — whisper's
                // cmake_minimum_required predates CMP0091, so the variable
                // alone is ignored and /MD stays baked into the flags.
                "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded",
                "-DCMAKE_C_FLAGS_RELEASE=/MT /O2 /Ob2 /DNDEBUG",
                "-DCMAKE_CXX_FLAGS_RELEASE=/MT /O2 /Ob2 /DNDEBUG",
                "-DCMAKE_BUILD_TYPE=Release",
            ])
            .status()
            .expect("run cmake configure");
        if !status.success() {
            panic!("cmake configure failed");
        }
    }

    let status = Command::new(&cmake)
        .arg("--build")
        .arg(build_dir)
        .arg("--config")
        .arg("Release")
        .arg("--target")
        .arg("whisper")
        .status()
        .expect("run cmake build");
    if !status.success() {
        panic!("cmake build failed");
    }
}

fn collect_lib_dirs(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name == "release" || name == "lib" || depth < 3 {
                out.push(path.clone());
            }
            collect_lib_dirs(&path, out, depth + 1);
        }
    }
}
