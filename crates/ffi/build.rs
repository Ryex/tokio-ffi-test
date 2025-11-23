use std::{
    collections::HashSet,
    env,
    io::Write,
    path::{Path, PathBuf},
};

use tinytemplate::TinyTemplate;
use tinytemplate::error::Result as TTResult;

use zngur::Zngur;

fn main() {
    let cxx = env::var("CXX").unwrap_or("c++".to_owned());

    let crate_dir = build_rs::input::cargo_manifest_dir();
    let out_dir = build_rs::input::out_dir();

    let zng_dir = crate_dir.join("zng");

    build_rs::output::rerun_if_changed(&zng_dir);

    let main_zng = check_and_generate_zng(&zng_dir, &out_dir);

    let generated_cpp = out_dir.join("generated.cpp");
    let generated_h = out_dir.join("generated.h");
    let target_h = crate_dir
        .join("include")
        .join("task-ffi")
        .join("generated.h");

    Zngur::from_zng_file(&main_zng)
        .with_cpp_file(&generated_cpp)
        .with_h_file(&generated_h)
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

    let headers: Vec<&str> = vec![
        "include/task-ffi/task.hpp",
        "include/task-ffi/types.hpp",
        "include/task-ffi/util.hpp",
    ];
    for header in &headers {
        build_rs::output::rerun_if_changed(header);
    }

    #[rustfmt::skip]
    let sources: Vec<&str> = vec![
        "src/task.cpp",
        "src/impl.cpp",
    ];

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
        "Parsed Zengur Spec (types: {}, funcs: {}, traits: {})",
        spec.types.len(),
        spec.funcs.len(),
        spec.traits.len(),
    ));

    let collect_missing = |spec: &zngur_def::ZngurSpec| {
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
        spec.traits.iter().for_each(|(trt, zngur_trt)| {
            if let zngur_def::RustTrait::Fn { inputs, output, .. } = trt {
                inputs.iter().chain([output.as_ref()]).for_each(|ref_ty| {
                    if ty_missing(ref_ty) {
                        insert_missing(ref_ty, &mut missing);
                    }
                });
            }
            zngur_trt.methods.iter().for_each(|trt_method| {
                trt_method
                    .inputs
                    .iter()
                    .chain([&trt_method.output])
                    .for_each(|ref_ty| {
                        if ty_missing(ref_ty) {
                            insert_missing(ref_ty, &mut missing);
                        }
                    });
            });
        });
        missing
    };

    let collect_result_types = |spec: &zngur_def::ZngurSpec| {
        let mut result_types = HashSet::new();

        spec.types.iter().for_each(|ty| {
            if let zngur_def::RustType::Adt(adt) = &ty.ty
                && let ["std", "result", "Result"] =
                    &adt.path.iter().map(String::as_str).collect::<Vec<_>>()[..]
            {
                let [t, err] = adt.generics.iter().collect::<Vec<_>>()[..] else {
                    build_rs::output::error(&format!(
                        "More than two generics for Result Type `{adt}`"
                    ));
                    unreachable!();
                };
                result_types.insert((t.clone(), err.clone(), LayoutPolicy(ty.layout)));
            }
        });
        result_types
    };

    let missing_bindings = collect_missing(&spec);
    let result_types = collect_result_types(&spec);

    let mut tt = TinyTemplate::new();
    tt.set_default_formatter(&tinytemplate::format_unescaped);
    wellknown_templates::register_templates(&mut tt);

    let bindings = missing_bindings
        .iter()
        .filter_map(call_with_tt(&tt, &bind_wellknown_missing_zng))
        .chain(
            result_types
                .iter()
                .map(call_with_tt(&tt, &bind_result_inspection)),
        )
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
        "Parsed Final Zengur Spec (types: {}, funcs: {}, traits: {})",
        final_spec.types.len(),
        final_spec.funcs.len(),
        final_spec.traits.len(),
    ));

    let missing_bindings = collect_missing(&final_spec);
    if !missing_bindings.is_empty() {
        missing_bindings.iter().for_each(|missing| {
            build_rs::output::error(&format!("Missing binding in final spec for `{missing}`",));
        });
        panic!("There are missing zngur type bindings that could not be generated!");
    }

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

    final_spec.funcs.iter().for_each(|func| {
        func.inputs.iter().chain([&func.output]).for_each(|ref_ty| {
            if matches!(ref_ty, zngur_def::RustType::Raw(_, _)) {
                build_rs::output::warning(&format!(
                    "Final spec containes Raw type `{}` in function `{}` ",
                    &ref_ty, &func.path,
                ));
            }
            if matches!(
                ref_ty,
                zngur_def::RustType::Ref(_mut,  boxed)
                  if matches!(**boxed, zngur_def::RustType::Ref(_, _))
            ) {
                build_rs::output::error(&format!(
                    "Final spec containes double ref type `{}` in function `{}`",
                    &ref_ty, &func.path
                ));
            }
        });
    });

    final_spec.traits.iter().for_each(|(trt, zngur_trt)| {
        if let zngur_def::RustTrait::Fn {
            name: fn_name,
            inputs,
            output,
            ..
        } = trt
        {
            inputs.iter().chain([output.as_ref()]).for_each(|ref_ty| {
                if matches!(ref_ty, zngur_def::RustType::Raw(_, _)) {
                    build_rs::output::warning(&format!(
                        "Final spec containes Raw type `{}` in trait method `{}` ",
                        &ref_ty, &fn_name,
                    ));
                }
                if matches!(
                    ref_ty,
                    zngur_def::RustType::Ref(_mut,  boxed)
                      if matches!(**boxed, zngur_def::RustType::Ref(_, _))
                ) {
                    build_rs::output::error(&format!(
                        "Final spec containes double ref type `{}` in trait method `{}`",
                        &ref_ty, &fn_name
                    ));
                }
            });
        }
        zngur_trt.methods.iter().for_each(|trt_method| {
            trt_method
                .inputs
                .iter()
                .chain([&trt_method.output])
                .for_each(|ref_ty| {
                    if matches!(ref_ty, zngur_def::RustType::Raw(_, _)) {
                        build_rs::output::warning(&format!(
                            "Final spec containes Raw type `{}` in trait method `{}` ",
                            &ref_ty, &trt_method.name,
                        ));
                    }
                    if matches!(
                        ref_ty,
                        zngur_def::RustType::Ref(_mut,  boxed)
                          if matches!(**boxed, zngur_def::RustType::Ref(_, _))
                    ) {
                        build_rs::output::error(&format!(
                            "Final spec containes double ref type `{}` in trait method `{}`",
                            &ref_ty, &trt_method.name,
                        ));
                    }
                });
        });
    });
    main_zng
}

mod wellknown_templates {
    use super::*;
    use indoc::indoc;
    use serde::Serialize;

    macro_rules! templates {
        ($($name:ident {
            $($body:tt)*
        } =>
        $template:expr;
        )*) => {
            $(
                #[derive(Serialize)]
                #[allow(clippy::upper_case_acronyms)]
                pub struct $name {
                    $($body)*
                }
                impl $name {
                    pub fn register(tt: &mut TinyTemplate) {
                        tt.add_template(stringify!($name), $template).expect("invalid template");
                    }
                    pub fn render(&self, tt: &TinyTemplate) -> TTResult<String> {
                        tt.render(stringify!($name), &self)
                    }
                }
            )*
            pub fn register_templates(tt: &mut TinyTemplate) {
                $(
                    $name::register(tt);
                )*
            }
        };
    }

    templates! {
        BOX {
            pub ty: String,
        } => indoc! {r#"
            type Box< { ty } > \{
                #layout(size = 16, align = 8);
            }
        "#};
        SLICE {
            pub ty: String,
            pub get_access: bool,
            pub pointer_access: bool,
        } => indoc!{r#"
            type [{ ty }] \{
                wellknown_traits(?Sized);

                {{ if pointer_access }}
                fn as_ptr(&self) -> *const { ty };
                {{ endif }}
                
                fn len(&self) -> usize;
                
                {{ if get_access }}
                fn get(&self, usize) -> ::std::option::Option<&{ ty } >;
                fn get_unchecked(&self, usize) -> &{ ty };
                {{ endif }}

            }

            {{ if get_access }}
            type ::std::option::Option<& { ty } > \{
                #layout(size = 8, align = 8);
                wellknown_traits(Debug, Copy);
                
                constructor None;
                constructor Some(&{ ty });

                fn unwrap(self) -> &{ ty };
                fn expect(self, &str) -> &{ ty };
            }
            {{ endif }}

            {{ if pointer_access }}
            mod ::std::slice \{
                fn from_raw_parts<{ ty }>(*const { ty }, usize) -> &[{ ty }];
            }
            {{ endif }}
        "#};
        VEC {
            pub ty: String,
            pub get_access: bool,
            pub pointer_access: bool,
        } => indoc! {r#"
            type ::std::vec::Vec< { ty } > \{
                #layout(size = 24, align = 8);
                
                fn new() -> ::std::vec::Vec< { ty } >;
                fn with_capacity(usize) -> ::std::vec::Vec< { ty } >;

                fn clone(&self) -> ::std::vec::Vec< { ty } >;

                fn push(&mut self, { ty });

                {{ if pointer_access }}
                fn as_ptr(&self) -> *const { ty };
                {{ endif }}

                fn as_slice(&self) -> &[{ ty }];

                fn capacity(&self) -> usize;
                fn reserve(&mut self, usize);

                fn len(&self) -> usize;

                {{ if get_access }}
                fn get(&self, usize) -> ::std::option::Option<& { ty } > deref [{ ty }];
                fn get_unchecked(&self, usize) -> &{ ty } deref [{ ty }];
                {{ endif }}


                fn extend_from_slice(&mut self, &[{ ty }]);
            }

            {{ if get_access }}
            type ::std::option::Option<& { ty } > \{
                #layout(size = 8, align = 8);
                wellknown_traits(Debug, Copy);
                
                constructor None;
                constructor Some(&{ ty });

                fn unwrap(self) -> &{ ty };
                fn expect(self, &str) -> &{ ty };
            }
            {{ endif }}

            type [{ ty }] \{
                wellknown_traits(?Sized);

                {{ if pointer_access }}
                fn as_ptr(&self) -> *const { ty };
                {{ endif }}

                fn len(&self) -> usize;
                
                {{ if get_access }}
                fn get(&self, usize) -> ::std::option::Option<& { ty } >;
                {{ endif }}

                fn get_unchecked(&self, usize) -> &{ ty };

                fn to_vec(&self) -> ::std::vec::Vec< { ty } >;
            }

            {{ if pointer_access }}
            mod ::std::slice \{
                fn from_raw_parts<{ ty }>(*const { ty }, usize) -> &[{ ty }];
            }
            {{ endif }}
        "#};
        OPTION {
            pub ty: String,
            pub size: usize,
            pub is_ref: bool,
            pub is_primitive: bool,
        } => indoc! {r#"
            type ::std::option::Option<{{ if is_ref }}&{{ endif }} { ty } > \{
                #layout(size = { size }, align = 8);
                {{ if is_primitive }}
                wellknown_traits(Debug, Copy);
                {{ endif }}
                
                constructor None;
                constructor Some({{ if is_ref }}&{{ endif }} { ty });

                fn unwrap(self) -> {{ if is_ref }}&{{ endif }} { ty };
                fn expect(self, &str) -> {{ if is_ref }}&{{ endif }} { ty };
            }
        "#};
        STRING {} => indoc! {r#"
            mod std \{

                mod string \{
                    type String \{
                        #layout(size = 24, align = 8);
                    
                        fn as_bytes(&self) -> &[u8];
                        fn as_mut_str(&mut self) -> &mut str;
                        fn as_str(&self) -> &str;
                        fn push_str(&mut self, &str);
                        fn capacity(&self) -> usize;
                        fn clear(&mut self);
                        fn reserve(&mut self, usize);
                        fn insert_str(&mut self, usize, &str);
                        fn len(&self) -> usize;
                        fn is_empty(&self) -> bool;

                        fn from_utf8(::std::vec::Vec<u8>) -> ::std::result::Result<::std::string::String, FromUtf8Error>;
                        fn from_utf16(&[u16]) -> ::std::result::Result<::std::string::String, FromUtf16Error>;
                        
                    }
                }

                
                mod option \{

                    type Option<&::std::string::String> \{
                        #layout(size = 8, align = 8);

                        fn unwrap(self) -> &::std::string::String;
                    }

                    type Option<::std::string::String> \{
                        #layout(size = 24, align = 8);

                        constructor None;
                        constructor Some(::std::string::String);

                        fn unwrap(self) -> ::std::string::String;
                    }
                }

                mod result \{
                    type Result<::std::string::String, ::std::string::FromUtf8Error> \{
                        #layout(size = 40, align = 8);
                        fn expect(self, &str) -> ::std::string::String;
                    }
                    type Result<::std::string::String, ::std::string::FromUtf16Error> \{
                        #layout(size = 24, align = 8);
                        fn expect(self, &str) -> ::std::string::String;
                    }
                    type Result<&str, ::std::str::Utf8Error> \{
                        #layout(size = 24, align = 8);

                        fn expect(self, &str) -> &str;
                    }
                }
            }
        "#};
        RESULT {
            pub ty: String,
            pub err: String,
            pub layout: String,
            pub inspect_access: bool,
            pub inspect_err_access: bool,
            pub is_debug_t: bool,
        } => indoc!{r#"
            type ::std::result::Result<{ ty }, { err }> \{
                {layout}

                constructor Ok({ ty });
                constructor Err({ err });

                fn unwrap(self) -> { ty };
                fn unwrap_unchecked(self) -> { ty };
                fn expect(self, &str) -> { ty };

                {{ if is_debug_t }}
                fn unwrap_err(self) -> { err };
                fn expect_err(self, &str) -> { err };
                {{ endif }}
                fn unwrap_err_unchecked(self) -> { err };

                fn is_ok(&self) -> bool;
                fn is_err(&self) -> bool;

                fn is_ok_and(self, Box<dyn Fn({ ty }) -> bool>) -> bool;
                fn is_err_and(self, Box<dyn Fn({ err }) -> bool>) -> bool;
                
                {{ if inspect_access }}
                fn inspect(self, Box<dyn Fn(&{ ty })> ) ->::std::result::Result<{ ty }, { err }>;
                {{ endif }}

                {{ if inspect_err_access }}
                fn inspect_err(self, Box<dyn Fn(&{ err })> ) ->::std::result::Result<{ ty }, { err }>;
                {{ endif }}
            }

            type Box<dyn Fn({ ty }) -> bool> \{
                #layout(size = 16, align = 8);
            }
            type Box<dyn Fn({ err }) -> bool> \{
                #layout(size = 16, align = 8);
            }

            {{ if inspect_access }}
            type Box<dyn Fn(&{ ty })> \{
                #layout(size = 16, align = 8);
            }
            {{ endif }}

            {{ if inspect_err_access }}
            type Box<dyn Fn(&{ err })> \{
                #layout(size = 16, align = 8);
            }
            {{ endif }}
            
        "#};
    }
}

fn call_with_tt<'a, 'func, T, Input, Func: Fn(&'a TinyTemplate, Input) -> T>(
    tt: &'a TinyTemplate,
    func: &'func Func,
) -> impl Fn(Input) -> T + use<'a, 'func, Input, T, Func> {
    |ty| func(tt, ty)
}

fn is_ref(ty: &zngur_def::RustType) -> bool {
    matches!(ty, zngur_def::RustType::Ref(_, _))
}

fn is_primitive(ty: &zngur_def::RustType) -> bool {
    matches!(ty, zngur_def::RustType::Primitive(_))
}

fn is_ref_primitive(ty: &zngur_def::RustType) -> bool {
    matches!(ty, zngur_def::RustType::Ref(_mut, boxed) if matches!(**boxed, zngur_def::RustType::Primitive(_)))
}

fn bind_wellknown_missing_zng(tt: &TinyTemplate, ty: &zngur_def::RustType) -> Option<String> {
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
                wellknown_templates::BOX {
                    ty: boxed.to_string(),
                }
                .render(tt)
                .unwrap(),
            );
        }
        zngur_def::RustType::Slice(boxed) => {
            build_rs::output::warning(&format!("Gnerating templated binding for `[{boxed}]`.",));
            return Some(
                wellknown_templates::SLICE {
                    ty: boxed.to_string(),
                    get_access: !is_ref(boxed),
                    pointer_access: is_primitive(boxed),
                }
                .render(tt)
                .unwrap(),
            );
        }
        zngur_def::RustType::Adt(ty_path) => {
            match &ty_path.path.iter().map(String::as_str).collect::<Vec<_>>()[..] {
                ["std", "string", "String"] => {
                    build_rs::output::warning(
                        "Gnerating templated binding for `::std::string::String`.",
                    );
                    return Some((wellknown_templates::STRING {}).render(tt).unwrap());
                }
                ["std", "vec", "Vec"] => {
                    let generic = ty_path.generics.first().unwrap();
                    build_rs::output::warning(&format!(
                        "Gnerating templated binding for `::std::vec::Vec<{generic}>`. get_access: {}",
                        !is_ref(generic)
                    ));
                    return Some(
                        wellknown_templates::VEC {
                            ty: generic.to_string(),
                            get_access: !is_ref(generic),
                            pointer_access: is_primitive(generic),
                        }
                        .render(tt)
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
                            (wellknown_templates::OPTION {
                                ty: generic.to_string(),
                                size: size_for_option_rust_primative(p),
                                is_primitive: true,
                                is_ref: false,
                            })
                            .render(tt)
                            .unwrap(),
                        );
                    } else if let zngur_def::RustType::Ref(_muta, ty) = generic {
                        build_rs::output::warning(&format!(
                            "Gnerating templated binding for `::std::option::Option<{generic}>`.",
                        ));
                        return Some(
                            (wellknown_templates::OPTION {
                                ty: ty.to_string(),
                                size: 16,
                                is_primitive: matches!(**ty, zngur_def::RustType::Primitive(_)),
                                is_ref: true,
                            })
                            .render(tt)
                            .unwrap(),
                        );
                    } else {
                        build_rs::output::warning(&format!(
                            "Gnerating templated binding for `::std::option::Option<{generic}>`.",
                        ));
                        let ty_name = generic.to_string();
                        return Some(
                            (wellknown_templates::OPTION {
                                size: size_for_option_t(&ty_name),
                                ty: ty_name,
                                is_primitive: matches!(*ty, zngur_def::RustType::Primitive(_)),
                                is_ref: true,
                            })
                            .render(tt)
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

fn bind_result_inspection(
    tt: &TinyTemplate,
    ty_err: &(zngur_def::RustType, zngur_def::RustType, LayoutPolicy),
) -> String {
    let (ty, err, layout) = ty_err;

    let ty_name = ty.to_string();
    let err_name = err.to_string();
    let is_debug = t_is_debug(ty);
    let inspect_access = !is_ref(ty);
    let inspect_err_access = !is_ref(err);
    build_rs::output::warning(&format!(
        "Gnerating templated binding for `::std::result::Result<{ty}, {err}>`. inspect: {inspect_access} inspect_err: {inspect_err_access}, debug: {is_debug}",
    ));
    wellknown_templates::RESULT {
        layout: match layout.0 {
            zngur_def::LayoutPolicy::StackAllocated { size, align } => {
                format!("#layout(size = {size}, align = {align});")
            }
            zngur_def::LayoutPolicy::HeapAllocated => "#heap_allocated;".to_string(),
            zngur_def::LayoutPolicy::OnlyByRef => "#only_by_ref;".to_string(),
        },
        ty: ty_name,
        err: err_name,
        inspect_access,
        inspect_err_access,
        is_debug_t: is_debug,
    }
    .render(tt)
    .unwrap()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LayoutPolicy(pub zngur_def::LayoutPolicy);

impl std::hash::Hash for LayoutPolicy {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.0 {
            zngur_def::LayoutPolicy::StackAllocated { size, align } => {
                1.hash(state);
                size.hash(state);
                align.hash(state);
            }
            zngur_def::LayoutPolicy::HeapAllocated => {
                2.hash(state);
            }
            zngur_def::LayoutPolicy::OnlyByRef => {
                3.hash(state);
            }
        }
    }
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

fn size_for_option_t(ty: &str) -> usize {
    match ty {
        "::std::string::String" => 24,
        _ => {
            build_rs::output::error(&format!("Unknown size of Option<{}>", ty));
            // panic!("Unknown size of Option<{}>", ty);
            0
        }
    }
}

fn t_is_debug(ty: &zngur_def::RustType) -> bool {
    if is_primitive(ty) || is_ref_primitive(ty) {
        return true;
    }

    if known_debug(&ty.to_string()) {
        return true;
    }

    match ty {
        zngur_def::RustType::Ref(_, boxed)
        | zngur_def::RustType::Raw(_, boxed)
        | zngur_def::RustType::Boxed(boxed)
        | zngur_def::RustType::Slice(boxed) => t_is_debug(boxed),
        zngur_def::RustType::Adt(ty_path) => {
            match &ty_path.path.iter().map(String::as_str).collect::<Vec<_>>()[..] {
                ["std", "string", "String"] => true,
                ["std", "vec", "Vec"] => {
                    let generic = ty_path.generics.first().unwrap();
                    t_is_debug(generic)
                }
                ["std", "option", "Option"] => {
                    let generic = ty_path.generics.first().unwrap();
                    t_is_debug(generic)
                }
                _ => { false }
            }
        }
        zngur_def::RustType::Tuple(tys) => tys.iter().all(t_is_debug),
        _ => false,
    }
}

fn known_debug(ty: &str) -> bool {
    #[allow(clippy::match_like_matches_macro)]
    match ty {
        "crate::ffi::CxxAny" => true,
        _ => false,
    }
}
