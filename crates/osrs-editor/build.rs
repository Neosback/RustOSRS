//! Stamp reproducible workspace source/context identity, independent of checkout paths and Git dirt.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
};
fn collect(path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if path.is_file() {
        files.push(path.to_path_buf());
    } else if path.is_dir() {
        for entry in fs::read_dir(path)? {
            collect(&entry?.path(), files)?;
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("../..");
    let mut files = Vec::new();
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        ".cargo",
        "profiles",
    ] {
        collect(&root.join(name), &mut files)?;
    }
    for entry in fs::read_dir(root.join("crates"))? {
        let dir = entry?.path();
        for name in ["Cargo.toml", "build.rs", "src", "data"] {
            collect(&dir.join(name), &mut files)?;
        }
    }
    files.sort_by_cached_key(|p| {
        p.strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    });
    let mut hash = Sha256::new();
    hash.update(b"rustosrs-workspace-source-v1\0");
    for file in files {
        let relative = file
            .strip_prefix(&root)?
            .to_str()
            .ok_or("Non-UTF8 source path")?
            .replace('\\', "/");
        println!("cargo:rerun-if-changed={}", file.display());
        hash.update((relative.len() as u64).to_le_bytes());
        hash.update(relative.as_bytes());
        hash.update(fs::metadata(&file)?.len().to_le_bytes());
        let mut reader = fs::File::open(file)?;
        let mut buffer = [0; 16384];
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
    }
    // Watch every directory root too: a new module, asset, profile or Cargo configuration must
    // refresh the stamp even when none of the previously enumerated files changed.
    for directory in ["crates", "profiles", ".cargo"] {
        println!("cargo:rerun-if-changed={}", root.join(directory).display());
    }
    println!(
        "cargo:rustc-env=RUSTOSRS_SOURCE_SHA256={:x}",
        hash.finalize()
    );
    let rustc = Command::new(env::var("RUSTC")?).arg("--version").output()?;
    if !rustc.status.success() {
        return Err("Cannot identify rustc".into());
    }
    println!(
        "cargo:rustc-env=RUSTOSRS_RUSTC_VERSION={}",
        String::from_utf8(rustc.stdout)?.trim()
    );
    for name in ["TARGET", "PROFILE", "OPT_LEVEL"] {
        println!("cargo:rustc-env=RUSTOSRS_BUILD_{name}={}", env::var(name)?);
    }
    let mut features: Vec<_> = env::vars_os()
        .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
        .filter(|(k, _)| k.starts_with("CARGO_FEATURE_"))
        .collect();
    features.sort();
    println!(
        "cargo:rustc-env=RUSTOSRS_BUILD_FEATURES={}",
        features
            .into_iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    println!(
        "cargo:rustc-env=RUSTOSRS_BUILD_RUSTFLAGS={}",
        env::var("CARGO_ENCODED_RUSTFLAGS")
            .unwrap_or_default()
            .escape_debug()
    );
    Ok(())
}
