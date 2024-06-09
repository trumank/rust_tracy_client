use std::io::Write;

fn link_dependencies() {
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("linux" | "android") => println!("cargo:rustc-link-lib=dl"),
        Ok("freebsd" | "dragonfly") => println!("cargo:rustc-link-lib=c"),
        Ok("windows") => println!("cargo:rustc-link-lib=user32"),
        Ok(_) => {}
        Err(e) => {
            writeln!(::std::io::stderr(), "Unable to get target_os=`{e}`!")
                .expect("could not report the error");
            ::std::process::exit(0xfd);
        }
    }

    let tool = cc::Build::default().get_compiler();
    let lib = if tool.is_like_gnu() {
        "libstdc++.a"
    } else if tool.is_like_clang() {
        "libc++.a"
    } else {
        return;
    };

    let output = tool
        .to_command()
        .arg("--print-file-name")
        .arg(lib)
        .output()
        .unwrap();
    if !output.status.success() || output.stdout.is_empty() {
        return;
    }
    let path = match std::str::from_utf8(&output.stdout) {
        Ok(path) => std::path::PathBuf::from(path),
        Err(_) => return,
    };
    if !path.is_absolute() {
        return;
    }
    println!(
        "cargo:rustc-link-search=native={}",
        path.parent().unwrap().display()
    );
    println!("cargo:rustc-link-lib=static=stdc++");
}

fn set_feature_defines(mut c: cc::Build) -> cc::Build {
    if std::env::var_os("CARGO_FEATURE_ENABLE").is_some() {
        c.define("TRACY_ENABLE", None);
    }
    if std::env::var_os("CARGO_FEATURE_TIMER_FALLBACK").is_some() {
        c.define("TRACY_TIMER_FALLBACK", None);
    }
    if std::env::var_os("CARGO_FEATURE_ONDEMAND").is_some() {
        c.define("TRACY_ON_DEMAND", None);
    }
    if std::env::var_os("CARGO_FEATURE_ONLY_LOCALHOST").is_some() {
        c.define("TRACY_ONLY_LOCALHOST", None);
    }
    if std::env::var_os("CARGO_FEATURE_ONLY_IPV4").is_some() {
        c.define("TRACY_ONLY_IPV4", None);
    }
    if std::env::var_os("CARGO_FEATURE_FIBERS").is_some() {
        c.define("TRACY_FIBERS", None);
    }
    if std::env::var_os("CARGO_FEATURE_MANUAL_LIFETIME").is_some() {
        c.define("TRACY_MANUAL_LIFETIME", None);
    }
    if std::env::var_os("CARGO_FEATURE_DELAYED_INIT").is_some() {
        c.define("TRACY_DELAYED_INIT", None);
    }
    if std::env::var_os("CARGO_FEATURE_FLUSH_ON_EXIT").is_some() {
        c.define("TRACY_NO_EXIT", None);
    }

    // Note: these are inversed and check for `is_none`!
    if std::env::var_os("CARGO_FEATURE_SYSTEM_TRACING").is_none() {
        c.define("TRACY_NO_SYSTEM_TRACING", None);
    }
    if std::env::var_os("CARGO_FEATURE_CONTEXT_SWITCH_TRACING").is_none() {
        c.define("TRACY_NO_CONTEXT_SWITCH", None);
    }
    if std::env::var_os("CARGO_FEATURE_SAMPLING").is_none() {
        c.define("TRACY_NO_SAMPLING", None);
    }
    if std::env::var_os("CARGO_FEATURE_CODE_TRANSFER").is_none() {
        c.define("TRACY_NO_CODE_TRANSFER", None);
    }
    if std::env::var_os("CARGO_FEATURE_BROADCAST").is_none() {
        c.define("TRACY_NO_BROADCAST", None);
    }
    if std::env::var_os("CARGO_FEATURE_CALLSTACK_INLINES").is_none() {
        c.define("TRACY_NO_CALLSTACK_INLINES", None);
    }
    c
}

fn build_tracy_client() {
    if std::env::var_os("CARGO_FEATURE_ENABLE").is_some() {
        let mut builder = set_feature_defines(cc::Build::new());
        let _ = builder
            .file("tracy/TracyClient.cpp")
            .warnings(false)
            .cpp(true);
        if let Ok(tool) = builder.try_get_compiler() {
            if tool.is_like_gnu() || tool.is_like_clang() {
                // https://github.com/rust-lang/cc-rs/issues/855
                builder.flag("-std=c++11");
            }
        }
        let _ = builder.try_flags_from_environment("TRACY_CLIENT_SYS_CXXFLAGS");

        let tool = cc::Build::default().get_compiler();

        if tool.is_like_msvc() {
            builder.flag("/MT");
        };

        builder.compile("libtracy-client.a");
        link_dependencies();
    }
}

fn main() {
    if let Ok(lib) = std::env::var("TRACY_CLIENT_LIB") {
        if let Ok(lib_path) = std::env::var("TRACY_CLIENT_LIB_PATH") {
            println!("cargo:rustc-link-search=native={lib_path}");
        }
        let kind = std::env::var_os("TRACY_CLIENT_STATIC");
        let mode = if kind.is_none() || kind.as_deref() == Some(std::ffi::OsStr::new("0")) {
            "dylib"
        } else {
            link_dependencies();
            "static"
        };
        println!("cargo:rustc-link-lib={mode}={lib}");
    } else {
        build_tracy_client();
    }
}
