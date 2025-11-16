use std::{env, io::Write};

use zngur::Zngur;

fn main() {
    let zng_files = ["std.zng", "ffi.zng", "tokio.zng", "tasks.zng"];

    // build::rerun_if_changed("ffi_impls.cpp");

    let cxx = env::var("CXX").unwrap_or("c++".to_owned());

    let crate_dir = build_rs::input::cargo_manifest_dir();
    let out_dir = build_rs::input::out_dir();

    let zng_dir = crate_dir.join("zng");

    let main_zng_path = out_dir.join("main.zng");
    {
        let mut main_zng =
            std::fs::File::create(&main_zng_path).expect("could not opem main.zng out file");

        for f in zng_files {
            let path = zng_dir.join(f);
            build_rs::output::rerun_if_changed(&path);

            let file = std::fs::File::open(&path)
                .unwrap_or_else(|err| panic!("Can not open {path:?}: {err}"));
            let reader = std::io::BufReader::new(file);

            write!(&mut main_zng, "// {0:->80}\n// {f: ^80}\n// {0:->80}\n", "")
                .unwrap_or_else(|err| panic!("error writing to output main.zng: {err}"));

            let mut inside_include = false;
            for (line_no, line) in std::io::BufRead::lines(reader).enumerate() {
                let line_no = line_no + 1;
                let line = line.unwrap_or_else(|err| {
                    panic!("Failed to read line {line_no} of {path:?}:  {err}")
                });

                if !inside_include && line.starts_with("#cpp_additional_includes") {
                    inside_include = true;
                } else if inside_include && line.trim().ends_with("\"") {
                    inside_include = false;
                }

                if !inside_include {
                    writeln!(&mut main_zng, "{line: <80} // {f}:{line_no}")
                        .unwrap_or_else(|err| panic!("error writing to output main.zng: {err}"));
                } else {
                    writeln!(&mut main_zng, "{line}")
                        .unwrap_or_else(|err| panic!("error writing to output main.zng: {err}"));
                }
            }
        }
    }

    let generated_cpp = out_dir.join("generated.cpp");
    let generated_h = out_dir.join("generated.h");
    let target_h = crate_dir
        .join("include")
        .join("tasks-ffi")
        .join("generated.h");

    Zngur::from_zng_file(&main_zng_path)
        .with_cpp_file(&generated_cpp)
        .with_h_file(out_dir.join("generated.h"))
        .with_rs_file(out_dir.join("generated.rs"))
        .generate();

    println!("copying {generated_h:?} to {target_h:?}");

    std::fs::copy(&generated_h, &target_h)
        .unwrap_or_else(|err| panic!("error copying generated header: {err}"));

    let build = &mut cc::Build::new();
    let build = build
        .cpp(true)
        .std("c++17")
        .compiler(&cxx)
        .include(crate_dir.join("include"))
        .include(&crate_dir);
    // .include(&out_dir);
    let build = || build.clone();
    // file may not exist if zngur determines it's not needed

    let headers = ["include/tasks-ffi/tasks.hpp", "include/tasks-ffi/types.hpp"];
    for header in &headers {
        build_rs::output::rerun_if_changed(header);
    }

    let sources: Vec<&str> = vec!["src/tasks.cpp"];

    let mut tasksffi = build();
    tasksffi.files(&sources);

    if generated_cpp.exists() {
        tasksffi.file(&generated_cpp);
    }

    for src in &sources {
        build_rs::output::rerun_if_changed(src);
    }

    tasksffi.compile("tasksffi")
}
