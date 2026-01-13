use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{ffi::OsStr, fs};
use walkdir::WalkDir;

// There are some files from this package that we need
// in packages that want to use this package.
// To pass these files we write them into the OUT_DIR
// from the build script. e.g. package a depends on rust-witness
// and calls transpile_wasm. The below files are writting into a/target/build
//
// TODO: figure out how to communicate between build scripts?

const W2C2_BUILD_SCRIPT: &str = include_str!("../build_w2c2.sh");
const GLOBALS_C: &str = include_str!("./globals.c");

// Get a function to spawn w2c2, either from $PATH, static binary, or by building locally
// Returns: (w2c2_command_fn, headers_path)
fn w2c2_cmd() -> (Box<dyn Fn() -> Command>, PathBuf) {
    let w2c2_path = Path::new(env::var("OUT_DIR").unwrap().as_str()).join(Path::new("w2c2"));
    // Use path relative to Cargo.toml for w2c2 headers
    let cargo_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let w2c2_headers_path = cargo_dir.join("w2c2");
    let w2c2_script_path =
        Path::new(env::var("OUT_DIR").unwrap().as_str()).join(Path::new("build_w2c2.sh"));
    
    // Check if headers are provided via environment variable
    let (headers_path, has_custom_headers) = if let Ok(custom_headers) = env::var("W2C2_HEADERS") {
        let custom_path = Path::new(&custom_headers);
        if custom_path.exists() && custom_path.is_dir() {
            (custom_path.to_path_buf(), true)
        } else {
            (w2c2_headers_path.clone(), false)
        }
    } else {
        (w2c2_headers_path.clone(), false)
    };
    
    // Check if headers already exist (from previous clone or custom path)
    let need_headers = !has_custom_headers && (!headers_path.exists() || !headers_path.join("w2c2_base.h").exists());
    
    fs::write(&w2c2_script_path, W2C2_BUILD_SCRIPT).expect("Failed to write build script");
    
    match Command::new("w2c2").spawn() {
        Ok(_) => {
            // w2c2 is in PATH, only clone if headers are needed and not provided via env
            if need_headers {
            Command::new("sh")
                .arg(w2c2_script_path.to_str().unwrap())
                .arg("1")
                .spawn()
                .expect("Failed to spawn w2c2 build")
                .wait()
                .expect("w2c2 build errored");
            }
            // Run the binary in the PATH
            (Box::new(|| Command::new("w2c2")), headers_path)
        }
        Err(_e) => {
            // Check for static binary via environment variable
            if let Ok(static_binary_path) = env::var("W2C2_BINARY") {
                let static_path = Path::new(&static_binary_path);
                if static_path.exists() && static_path.is_file() {
                    // Only clone if headers are needed and not provided via env
                    if need_headers {
                        Command::new("sh")
                            .arg(w2c2_script_path.to_str().unwrap())
                            .arg("1")
                            .spawn()
                            .expect("Failed to spawn w2c2 build")
                            .wait()
                            .expect("w2c2 build errored");
                    }
                    let static_path_str = static_path.to_str().unwrap().to_string();
                    return (
                        Box::new(move || Command::new(&static_path_str)),
                        headers_path,
                    );
                }
            }
            
            // Check if static binary already exists in OUT_DIR
            let static_binary = Path::new(env::var("OUT_DIR").unwrap().as_str()).join("w2c2-static");
            if static_binary.exists() && static_binary.is_file() {
                // Only clone if headers are needed and not provided via env
                if need_headers {
                    Command::new("sh")
                        .arg(w2c2_script_path.to_str().unwrap())
                        .arg("1")
                        .spawn()
                        .expect("Failed to spawn w2c2 build")
                        .wait()
                        .expect("w2c2 build errored");
                }
                let static_binary_str = static_binary.to_str().unwrap().to_string();
                return (
                    Box::new(move || Command::new(&static_binary_str)),
                    headers_path,
                );
            }
            
            // Build the w2c2 binary from source (fallback)
            Command::new("sh")
                .arg(w2c2_script_path.to_str().unwrap())
                .spawn()
                .expect("Failed to spawn w2c2 build")
                .wait()
                .expect("w2c2 build errored");
            (
                Box::new(|| {
                    let w2c2_path =
                        Path::new(env::var("OUT_DIR").unwrap().as_str()).join(Path::new("w2c2"));
                    let w2c2_exec_path = w2c2_path.join(Path::new("build/w2c2/w2c2"));
                    Command::new(w2c2_exec_path.to_str().unwrap())
                }),
                headers_path,
            )
        }
    }
}

pub fn transpile_wasm(wasmdir: String) {
    let globals_c_path = Path::new(&env::var("OUT_DIR").unwrap()).join(Path::new("globals.c"));
    fs::write(&globals_c_path, GLOBALS_C).expect("Failed to write globals.c");
    if !Path::is_dir(Path::new(wasmdir.as_str())) {
        panic!("wasmdir must be a directory");
    }
    println!("cargo:rerun-if-changed={}", wasmdir);

    let (w2c2, w2c2_path) = w2c2_cmd();

    let circuit_out_dir = env::var("OUT_DIR").unwrap();
    let mut builder = cc::Build::new();
    // empty the handlers file
    let mut handler = "".to_string();
    builder
        .file(globals_c_path.to_str().unwrap())
        .file(
            Path::new(circuit_out_dir.as_str())
                .join(Path::new("handlers.c"))
                .to_str()
                .unwrap(),
        )
        .flag(format!("-I{}", w2c2_path.to_str().unwrap()).as_str())
        .flag("-Wno-unused-label")
        .flag("-Wno-unused-but-set-variable")
        .flag("-Wno-unused-variable")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-null-character")
        .flag("-Wno-c2x-extensions");

    let mut last_modified_file = std::time::SystemTime::UNIX_EPOCH;
    for entry in WalkDir::new(wasmdir) {
        let e = entry.unwrap();
        let path = e.path();
        if path.is_dir() {
            continue;
        }
        let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");
        // Iterate over all wasm files and generate c source, then compile each source to
        // a static library that can be called from rust
        if ext != "wasm" {
            continue;
        }
        // make source files with the same name as the wasm binary file
        let circuit_name = path.file_stem().unwrap();
        let circuit_name_compressed = circuit_name
            .to_str()
            .unwrap()
            .replace("_", "")
            .replace("-", "");
        handler.push_str(
            format!(
                "void {}_runtime__exceptionHandler(void*) {{ }}\n",
                circuit_name_compressed
            )
            .as_str(),
        );
        handler.push_str(
            format!(
                "void {}_runtime__printErrorMessage(void*) {{ }}\n",
                circuit_name_compressed
            )
            .as_str(),
        );
        // w2c2 is using a fixed naming convention when splitting source by the number of functions ("-f n" flag).
        // The output files are named s00..01.c, s00..02.c, s00..03.c, etc., and a main file named after the wasm file.
        // As there may be multiple wasm files, we need to transpile each wasm file into a separate directory to prevent
        // w2c2 from overwriting the s..x.c files.

        let circuit_out_dir =
            Path::new(&circuit_out_dir).join(Path::new(circuit_name.to_str().unwrap()));

        if !circuit_out_dir.exists() {
            fs::create_dir(&circuit_out_dir).expect("Failed to create circuit output directory");
        }

        let out = Path::new(&circuit_out_dir)
            .join(Path::new(path.file_name().unwrap()))
            .with_extension("c");
        // Check if the source file needs to be regenerated
        if needs_regeneration(path, &out) {
            // first generate the c source
            w2c2()
                .arg("-p")
                .arg("-m")
                .arg("-f 1")
                .arg(path)
                .arg(out.clone())
                .spawn()
                .expect("Failed to spawn w2c2")
                .wait()
                .expect("w2c2 command errored");

            let contents = fs::read_to_string(out.clone()).unwrap();
            // make the data constants static to prevent duplicate symbol errors
            fs::write(
                out.clone(),
                contents.replace("const U8 d", "static const U8 d"),
            )
            .expect("Error modifying data symbols");
        } else {
            println!(
                "C source files are up to date, skipping transpilation: {}",
                path.display()
            );
            last_modified_file = std::cmp::max(
                last_modified_file,
                fs::metadata(&out)
                    .expect("Failed to read metadata")
                    .modified()
                    .expect("Failed to read modified time"),
            );
        }

        builder.file(out.clone());
        // Add all the files to the builder that start with "s0..." and end with ".c" (the results of w2c2 `-f` flag)
        for entry in WalkDir::new(circuit_out_dir.clone()) {
            let e = entry.unwrap();
            let path = e.path();
            if path.is_dir() {
                continue;
            }
            let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");
            if ext != "c" {
                continue;
            }
            if path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("s0")
                && path.file_name().unwrap().to_str().unwrap().ends_with(".c")
            {
                builder.file(path);
            }
        }
    }

    let handlers = Path::new(circuit_out_dir.as_str()).join("handlers.c");
    fs::write(handlers, handler).expect("Error writing handler source");

    builder.compile("circuit");
}

fn needs_regeneration(source: &Path, generated: &Path) -> bool {
    if !generated.exists() {
        return true;
    }
    let source_metadata = fs::metadata(source).expect("Failed to read source metadata");
    let generated_metadata = fs::metadata(generated).expect("Failed to read generated metadata");

    let source_modified = source_metadata
        .modified()
        .expect("Failed to read source modification time");
    let generated_modified = generated_metadata
        .modified()
        .expect("Failed to read generated modification time");

    source_modified > generated_modified
}
