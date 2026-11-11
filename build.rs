use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_rc_exe() -> Option<PathBuf> {
    if let Ok(output) = Command::new("where.exe").arg("rc.exe").output() {
        if output.status.success() {
            if let Some(first) = String::from_utf8_lossy(&output.stdout).lines().next() {
                let p = PathBuf::from(first.trim());
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    let kits_bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if let Ok(entries) = std::fs::read_dir(kits_bin) {
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path().join("x64").join("rc.exe"))
            .filter(|p| p.exists())
            .collect();
        versions.sort();
        if let Some(latest) = versions.pop() {
            return Some(latest);
        }
    }
    None
}

fn main() {
    println!("cargo:rerun-if-changed=assets/kurowser.ico");
    println!("cargo:rerun-if-changed=assets/kurowser.manifest");
    println!("cargo:rerun-if-changed=assets/kurowser.rc");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let res_path = out_dir.join("kurowser.res");

    if let Some(rc) = find_rc_exe() {
        let status = Command::new(rc)
            .args(["/nologo", "/fo"])
            .arg(&res_path)
            .arg("assets/kurowser.rc")
            .status();

        if let Ok(s) = status {
            if s.success() && res_path.exists() {
                println!("cargo:rustc-link-arg={}", res_path.display());
            }
        }
    }
}

