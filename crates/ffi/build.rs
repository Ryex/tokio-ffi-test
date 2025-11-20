use std::{
    collections::HashSet,
    env,
    io::Write,
    path::{Path, PathBuf},
};

use zngur::Zngur;

fn main() {

    let cxx = env::var("CXX").unwrap_or("c++".to_owned());

    let crate_dir = build_rs::input::cargo_manifest_dir();
    let out_dir = build_rs::input::out_dir();

    let zng_dir = crate_dir.join("zng");

    build_rs::output::rerun_if_changed(&zng_dir);

    let main_zng = check_and_generate_zng(&zng_dir, &out_dir);

    let generated_cpp = out_dir.join("zngur_generated.cpp");
    let generated_h = out_dir.join("zngur_generated.h");
    let target_h = crate_dir
        .join("include")
        .join("task-ffi")
        .join("zngur_generated.h");

    Zngur::from_zng_file(&main_zng)
        .with_cpp_file(&generated_cpp)
        .with_h_file(out_dir.join("zngur_generated.h"))
        .with_rs_file(out_dir.join("zngur_generated.rs"))
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

    let headers = ["include/task-ffi/task.hpp", "include/task-ffi/types.hpp", "include/task-ffi/util.hpp"];
    for header in &headers {
        build_rs::output::rerun_if_changed(header);
    }

    let sources: Vec<&str> = vec!["src/task.cpp", "src/impl.cpp"];

    let mut taskffi = build();
    taskffi.files(&sources);

    if generated_cpp.exists() {
        taskffi.file(&generated_cpp);
    }

    for src in &sources {
        build_rs::output::rerun_if_changed(src);
    }

    taskffi.compile("taskffi")
}

fn check_and_generate_zng(zng_path: &Path, out_path: &Path) -> PathBuf {
    let main_zng = zng_path.join("main.zng");
    let spec = zngur_parser::ParsedZngFile::parse(main_zng);
    build_rs::output::warning(&format!(
        "Parsed Zengur Spec (types: {}, funcs: {})",
        spec.types.len(),
        spec.funcs.len()
    ));

    let types: HashSet<String> = spec.types.iter().map(|ty| ty.ty.to_string()).collect();
    let ty_missing = |ty: &zngur_def::RustType| {
        fn missing(ty: &zngur_def::RustType, types: &HashSet<String>) -> bool {
            match ty {
                zngur_def::RustType::Primitive(_) => false,
                zngur_def::RustType::Ref(_, boxed) | zngur_def::RustType::Raw(_, boxed) => {
                    missing(boxed, types)
                }
                zngur_def::RustType::Tuple(items) if items.is_empty() => false,
                _ => !types.contains(&ty.to_string()),
            }
        }
        missing(ty, &types)
    };

    let collect_missing = |spec: &zngur_def::ZngurSpec| {
        fn insert_missing(ty: &zngur_def::RustType, missing: &mut HashSet<zngur_def::RustType>) {
            match ty {
                zngur_def::RustType::Ref(_, boxed) | zngur_def::RustType::Raw(_, boxed) => {
                    insert_missing(boxed, missing);
                }
                ty => {
                    missing.insert(ty.clone());
                }
            }
        }

        let mut missing: HashSet<zngur_def::RustType> = HashSet::new();

        spec.types.iter().for_each(|ty| {
            ty.methods.iter().for_each(|method| {
                method
                    .data
                    .inputs
                    .iter()
                    .chain([&method.data.output])
                    .for_each(|ref_ty| {
                        if ty_missing(ref_ty) {
                            insert_missing(ref_ty, &mut missing);
                        }
                    });
            });
            ty.fields.iter().for_each(|f| {
                let ref_ty = &f.ty;
                if ty_missing(ref_ty) {
                    insert_missing(ref_ty, &mut missing);
                }
            });
        });
        spec.funcs.iter().for_each(|func| {
            func.inputs.iter().chain([&func.output]).for_each(|ref_ty| {
                if ty_missing(ref_ty) {
                    insert_missing(ref_ty, &mut missing);
                }
            });
        });
        missing
    };

    let missing_bindings = collect_missing(&spec);

    let bindings = missing_bindings
        .iter()
        .filter_map(bind_wellknown_missing_zng)
        .collect::<Vec<_>>();

    // copy zng to output dir for inclusion in generated main.zng
    let source_out_path = out_path.join("zng").join("source");
    std::fs::create_dir_all(&source_out_path)
        .expect("Failed to create copy location for source zng files");
    for entry in walkdir::WalkDir::new(zng_path)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let relative_path = match entry.path().strip_prefix(zng_path) {
            Ok(rp) => rp,
            Err(_) => panic!("strip_prefix failed; this is a probably a bug in copy_dir logic"),
        };

        let target_path = {
            let mut target_path = source_out_path.to_path_buf();
            target_path.push(relative_path);
            target_path
        };

        let source_metadata = match entry.metadata() {
            Err(_) => {
                build_rs::output::error(&format!("walkdir metadata error for {:?}", entry.path()));
                continue;
            }

            Ok(md) => md,
        };

        if source_metadata.is_dir() {
            if let Err(err) = std::fs::create_dir(&target_path) {
                build_rs::output::error(&err.to_string());
            }
            if let Err(err) = std::fs::set_permissions(&target_path, source_metadata.permissions())
            {
                build_rs::output::error(&err.to_string());
            }
        } else if let Err(err) = std::fs::copy(entry.path(), &target_path) {
            build_rs::output::error(&err.to_string());
        }
    }

    // write generated zng bindings
    {
        let mut binding_zng =
            std::fs::File::create(out_path.join("zng").join("generated_bindings.zng"))
                .expect("could not open generated_bindings.zng out file");

        for binding in bindings {
            writeln!(&mut binding_zng, "{binding}").unwrap_or_else(|err| {
                panic!("error writing to output generated_bindings.zng: {err}")
            });
        }
    }

    // write the new main.zng that imports generated bindings and old main, return path
    let main_zng = {
        let main_zng_path = out_path.join("zng").join("main.zng");
        let mut main_zng =
            std::fs::File::create(&main_zng_path).expect("could not open main.zng out file");

        writeln!(&mut main_zng, r#"import "./source/main.zng"; "#)
            .unwrap_or_else(|err| panic!("error writing to output main.zng: {err}"));
        writeln!(&mut main_zng, r#"import "./generated_bindings.zng"; "#)
            .unwrap_or_else(|err| panic!("error writing to output main.zng: {err}"));

        main_zng_path
    };

    let final_spec = zngur_parser::ParsedZngFile::parse(main_zng.to_path_buf());

    build_rs::output::warning(&format!(
        "Parsed Final Zengur Spec (types: {}, funcs: {})",
        final_spec.types.len(),
        final_spec.funcs.len()
    ));

    final_spec.types.iter().for_each(|ty| {
        if matches!(&ty.ty, zngur_def::RustType::Raw(_, _)) {
            build_rs::output::error(&format!("Final spec containes Raw type `{}`", &ty.ty));
        }
        if matches!(
            &ty.ty,
            zngur_def::RustType::Ref(_mut,  boxed)
              if matches!(**boxed, zngur_def::RustType::Ref(_, _))
        ) {
            build_rs::output::error(&format!(
                "Final spec containes double ref type `{}`",
                &ty.ty
            ));
        }
        ty.methods.iter().for_each(|method| {
            method
                .data
                .inputs
                .iter()
                .chain([&method.data.output])
                .for_each(|ref_ty| {
                    if matches!(ref_ty, zngur_def::RustType::Raw(_, _)) {
                        build_rs::output::warning(&format!(
                            "Final spec containes Raw type `{}` in method `{}` on `{}`",
                            &ref_ty, method.data.name, &ty.ty
                        ));
                    }
                    if matches!(
                        ref_ty,
                        zngur_def::RustType::Ref(_mut,  boxed)
                          if matches!(**boxed, zngur_def::RustType::Ref(_, _))
                    ) {
                        build_rs::output::error(&format!(
                            "Final spec containes double ref type `{}` in method `{}` on `{}`",
                            &ref_ty, method.data.name, &ty.ty
                        ));
                    }
                });
        });
    });

    main_zng
}

mod wellknown_templates {
    use sailfish::Template;

    #[derive(Template)]
    #[template(path = "std-string-String.stpl", escape = false)]
    pub struct StdString;

    #[derive(Template)]
    #[template(path = "std-vec-Vec.stpl", escape = false)]
    pub struct StdVec {
        pub ty: String,
        pub get_access: bool,
        pub pointer_access: bool,
    }

    #[derive(Template)]
    #[template(path = "std-option-Option.stpl", escape = false)]
    pub struct StdOption {
        pub ty: String,
        pub size: usize,
    }

    #[derive(Template)]
    #[template(path = "slice.stpl", escape = false)]
    pub struct Slice {
        pub ty: String,
        pub get_access: bool,
        pub pointer_access: bool,
    }

    #[derive(Template)]
    #[template(path = "box.stpl", escape = false)]
    pub struct Box {
        pub ty: String,
    }
}

fn bind_wellknown_missing_zng(ty: &zngur_def::RustType) -> Option<String> {
    use sailfish::Template;

    let is_ref = |ty: &zngur_def::RustType| matches!(ty, zngur_def::RustType::Ref(_, _));

    let is_primitive = |ty: &zngur_def::RustType| matches!(ty, zngur_def::RustType::Primitive(_));

    match ty {
        zngur_def::RustType::Primitive(_p) => {
            build_rs::output::warning(&format!(
                "Primative type {ty} detected as missing! (this should never happen)",
            ));
        }
        zngur_def::RustType::Ref(_, boxed) | zngur_def::RustType::Raw(_, boxed) => {
            build_rs::output::warning(&format!(
                "Can not build binding for missing refrence type `{boxed}`. Possible double ref `&&` in use? ",
            ));
        }
        zngur_def::RustType::Boxed(boxed) => {
            build_rs::output::warning(&format!("Gnerating templated binding for `Box<{boxed}>`.",));
            return Some(
                wellknown_templates::Box {
                    ty: boxed.to_string(),
                }
                .render()
                .unwrap(),
            );
        }
        zngur_def::RustType::Slice(boxed) => {
            build_rs::output::warning(&format!("Gnerating templated binding for `[{boxed}]`.",));
            return Some(
                wellknown_templates::Slice {
                    ty: boxed.to_string(),
                    get_access: !is_ref(boxed),
                    pointer_access: is_primitive(boxed),
                }
                .render()
                .unwrap(),
            );
        }
        zngur_def::RustType::Adt(ty_path) => {
            match &ty_path.path.iter().map(String::as_str).collect::<Vec<_>>()[..] {
                ["std", "string", "String"] => {
                    build_rs::output::warning(
                        "Gnerating templated binding for `::std::string::String`.",
                    );
                    return Some((wellknown_templates::StdString {}).render().unwrap());
                }
                ["std", "vec", "Vec"] => {
                    let generic = ty_path.generics.first().unwrap();
                    build_rs::output::warning(&format!(
                        "Gnerating templated binding for `::std::vec::Vec<{generic}>`.",
                    ));
                    return Some(
                        wellknown_templates::StdVec {
                            ty: generic.to_string(),
                            get_access: !is_ref(generic),
                            pointer_access: is_primitive(generic),
                        }
                        .render()
                        .unwrap(),
                    );
                }
                ["std", "option", "Option"] => {
                    let generic = ty_path.generics.first().unwrap();

                    if let zngur_def::RustType::Primitive(p) = generic {
                        build_rs::output::warning(&format!(
                            "Gnerating templated binding for `::std::option::Option<{generic}>`.",
                        ));
                        return Some(
                            (wellknown_templates::StdOption {
                                ty: generic.to_string(),
                                size: size_for_option_rust_primative(p),
                            })
                            .render()
                            .unwrap(),
                        );
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    build_rs::output::warning(&format!(
        "Missing type binding `{ty}`. Can not generate binding for this type."
    ));
    None
}

#[allow(dead_code)]
fn size_for_rust_primative(p: &zngur_def::PrimitiveRustType) -> usize {
    match p {
        zngur_def::PrimitiveRustType::Uint(u32) => match u32 {
            8 => std::mem::size_of::<u8>(),
            16 => std::mem::size_of::<u16>(),
            32 => std::mem::size_of::<u32>(),
            64 => std::mem::size_of::<u64>(),
            _ => 8,
        },
        zngur_def::PrimitiveRustType::Int(u32) => match u32 {
            8 => std::mem::size_of::<i8>(),
            16 => std::mem::size_of::<i16>(),
            32 => std::mem::size_of::<i32>(),
            64 => std::mem::size_of::<i64>(),
            _ => 8,
        },
        zngur_def::PrimitiveRustType::Float(u32) => match u32 {
            32 => std::mem::size_of::<f32>(),
            64 => std::mem::size_of::<f64>(),
            _ => 8,
        },
        zngur_def::PrimitiveRustType::Usize => std::mem::size_of::<usize>(),
        zngur_def::PrimitiveRustType::Bool => std::mem::size_of::<bool>(),
        zngur_def::PrimitiveRustType::Str => 0,
        zngur_def::PrimitiveRustType::ZngurCppOpaqueOwnedObject => 16,
    }
}

#[allow(dead_code)]
fn size_for_option_rust_primative(p: &zngur_def::PrimitiveRustType) -> usize {
    match p {
        zngur_def::PrimitiveRustType::Uint(u32) => match u32 {
            8 => std::mem::size_of::<Option<u8>>(),
            16 => std::mem::size_of::<Option<u16>>(),
            32 => std::mem::size_of::<Option<u32>>(),
            64 => std::mem::size_of::<Option<u64>>(),
            _ => 8,
        },
        zngur_def::PrimitiveRustType::Int(u32) => match u32 {
            8 => std::mem::size_of::<Option<i8>>(),
            16 => std::mem::size_of::<Option<i16>>(),
            32 => std::mem::size_of::<Option<i32>>(),
            64 => std::mem::size_of::<Option<i64>>(),
            _ => 8,
        },
        zngur_def::PrimitiveRustType::Float(u32) => match u32 {
            32 => std::mem::size_of::<Option<f32>>(),
            64 => std::mem::size_of::<Option<f64>>(),
            _ => 8,
        },
        zngur_def::PrimitiveRustType::Usize => std::mem::size_of::<Option<usize>>(),
        zngur_def::PrimitiveRustType::Bool => std::mem::size_of::<Option<bool>>(),
        zngur_def::PrimitiveRustType::Str => 0,
        zngur_def::PrimitiveRustType::ZngurCppOpaqueOwnedObject => 16,
    }
}
