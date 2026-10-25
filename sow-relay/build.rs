// build.rs — replicate the M3 link order for the single F-Stack relay worker.
//
// fstack-bridge emits its link flags as `rustc-link-lib` (so they reach this
// dependent crate), but the proven F-Stack order puts every
// DPDK archive + libfstack.a as raw link-args AFTER the rlib inputs. Archive
// position in the link line determines constructor (`.init_array`) ordering in
// the final binary, and F-Stack's kernel emulation is sensitive to it: with a
// different order, `lo_set_defaultaddr` fails and `ff_veth_attach` segfaults
// in `uma_zalloc_pcpu_arg`. Emit the same args here so this binary links
// exactly like the validated live relay build.
use std::process::Command;

fn main() {
    let fstack_lib_dir = std::env::var("FSTACK_LIB_DIR").unwrap_or_else(|_| {
        if std::path::Path::new("/usr/local/lib/libfstack.a").exists() {
            "/usr/local/lib".to_string()
        } else if std::path::Path::new("/home/azureuser/f-stack/lib/libfstack.a").exists() {
            "/home/azureuser/f-stack/lib".to_string()
        } else {
            "/opt/sow-dpdk/lib".to_string()
        }
    });
    println!("cargo:rerun-if-env-changed=FSTACK_LIB_DIR");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    println!("cargo:rerun-if-env-changed=SOW_RELAY_FOUNDATION_ID");
    println!("cargo:rerun-if-env-changed=SOW_RELAY_BUILD_GIT");
    for key in ["SOW_RELAY_FOUNDATION_ID", "SOW_RELAY_BUILD_GIT"] {
        if let Ok(value) = std::env::var(key) {
            println!("cargo:rustc-env={key}={value}");
        }
    }
    println!("cargo:rerun-if-changed=src/reload.c");
    if std::env::var_os("FSTACK_LIB_DIR").is_some() {
        let foundation = std::env::var("SOW_RELAY_FOUNDATION_ID")
            .expect("configured relay build needs its foundation identity");
        assert!(foundation.len() == 64 && foundation.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid relay foundation identity");
        let obj = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("reload.o");
        let flags = Command::new("pkg-config").args(["--cflags", "libdpdk"])
            .output().expect("cannot read native DPDK include flags");
        assert!(flags.status.success(), "native DPDK headers unavailable");
        let result = Command::new("cc").args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I"])
            .arg(&fstack_lib_dir).args(String::from_utf8_lossy(&flags.stdout).split_whitespace())
            .args(["-c", "src/reload.c", "-o"]).arg(&obj)
            .status().expect("cannot compile F-Stack reload adapter");
        assert!(result.success(), "F-Stack reload adapter compile failed");
        println!("cargo:rustc-link-arg={}", obj.display());
    }
    match Command::new("pkg-config").args(["--modversion", "libdpdk"]).output() {
        Ok(version) if version.status.success() => {
            let version = String::from_utf8_lossy(&version.stdout);
            assert_eq!(version.trim(), "24.11.8", "relay requires bundled DPDK 24.11.8");
            println!("cargo:rustc-env=SOW_DPDK_VERSION={}", version.trim());
        }
        _ if std::env::var_os("FSTACK_LIB_DIR").is_some() => {
            panic!("configured relay build cannot locate DPDK 24.11.8");
        }
        _ => {}
    }
    // Without rerun-if-changed on the archive, cargo treats the bin as fresh
    // when only libfstack.a is rebuilt on the build host — f-stack source
    // changes (e.g. the callout_when fix) get compiled into the lib but never
    // relinked into the deployed binary.
    println!("cargo:rerun-if-changed={fstack_lib_dir}/libfstack.a");
    let out = Command::new("pkg-config")
        .args(["--static", "--libs", "libdpdk"])
        .output();
    match out {
        Ok(out) if out.status.success() => {
            for tok in String::from_utf8_lossy(&out.stdout).split_whitespace() {
                println!("cargo:rustc-link-arg={}", tok);
            }

            println!("cargo:rustc-link-search=native={fstack_lib_dir}");

            // libfstack.a MUST be pulled in whole (fstack localizes all symbols
            // then re-exports only ff_api.symlist globals).
            println!("cargo:rustc-link-arg=-Wl,--whole-archive");
            println!("cargo:rustc-link-arg=-l:libfstack.a");
            println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");

            // Preserve DPDK constructor/init ELF sections.
            println!("cargo:rustc-link-arg=-Wl,-z,nostart-stop-gc");
            println!("cargo:rustc-link-arg=-Wl,--export-dynamic");

            // F-Stack runtime deps (resolved after libfstack.a).
            for lib in ["crypto", "z", "numa", "rt", "m", "dl"] {
                println!("cargo:rustc-link-arg=-l{lib}");
            }
            println!("cargo:rustc-link-arg=-pthread");
        }
        _ => {
            // Dev host without DPDK: nothing to link (cargo check still passes).
            eprintln!("build.rs: pkg-config libdpdk not found; skipping DPDK link flags");
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
}
