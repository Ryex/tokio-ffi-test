use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use tinytemplate::TinyTemplate;

use color_eyre::eyre::Result;
fn build_zng(crate_dir: &Path) -> Result<()> {
    let mut tt = TinyTemplate::new();
    let mut templates: HashMap<String, String> = HashMap::new();

    let template_dir = crate_dir.join("templates");

    let mut template_paths: Vec<PathBuf> = vec![template_dir.clone()];

    while let Some(path) = template_paths.pop() {
        let entries = std::fs::read_dir(path)?;
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if entry_path.is_file() && entry_path.ends_with(".zng.tt") {
                    let name = entry_path
                        .parent()
                        .map(|parent| {
                            parent
                                .strip_prefix(&template_dir)
                                .iter()
                                .map(|part| part.to_string_lossy())
                                .collect::<Vec<_>>()
                                .join("_")
                                + "_"
                        })
                        .unwrap_or_default()
                        + &entry_path
                            .file_stem()
                            .unwrap_or(&entry.file_name())
                            .to_string_lossy();
                    let content = std::fs::read_to_string(entry.path())?;
                    templates.insert(name, content);
                } else if entry_path.is_dir() {
                    template_paths.push(entry_path);
                }
            }
        }
    }

    for (name, template) in templates.iter() {
        tt.add_template(name, template);
    }

    let out_dir = build::out_dir();
    let main_zng_path = out_dir.join("main.zng");

    let mut main_zng = std::fs::File::create(&main_zng_path)?;

    link_options(&tt, &mut main_zng);

    Ok(())
}

trait NotCopy {
    const IS_COPY: bool = false;
}
impl<T: ?Sized> NotCopy for T {}
struct IsCopy<T: ?Sized>(std::marker::PhantomData<T>);
impl<T: ?Sized + Copy> IsCopy<T> {
    const IS_COPY: bool = true;
}

trait NotClone {
    const IS_CLONE: bool = false;
}
impl<T: ?Sized> NotClone for T {}
struct IsClone<T: ?Sized>(std::marker::PhantomData<T>);
impl<T: ?Sized + Clone> IsClone<T> {
    const IS_CLONE: bool = true;
}

trait NotDefault {
    const IS_DEFAULT: bool = false;
}
impl<T: ?Sized> NotDefault for T {}
struct IsDefault<T: ?Sized>(std::marker::PhantomData<T>);
impl<T: ?Sized + Default> IsClone<T> {
    const IS_DEFAULT: bool = true;
}

#[derive(serde::Serialize)]
struct TypeProps<'a> {
    ty: &'a str,
    size: usize,
    align: usize,
    is_copy: bool,
    is_clone: bool,
    is_sized: bool,
    is_default: bool,
}

trait IsSized {
    const IS_SIZED: bool;
}
impl<T> IsSized for T {
    const IS_SIZED: bool = true;
}
impl IsSized for str {
    const IS_SIZED: bool = false;
}
impl<T> IsSized for [T] {
    const IS_SIZED: bool = false;
}
impl IsSized for dyn std::any::Any + '_ {
    const IS_SIZED: bool = false;
}
const fn is_sized<T: ?Sized + IsSized>() -> bool {
    T::IS_SIZED
}

struct Sizer<T: ?Sized, const HAS_SIZE: bool>(std::marker::PhantomData<T>);

impl<T: ?Sized> Sizer<T, false>{
    fn size_of() -> usize {
        0
    }
    fn align_of() -> usize {
        0
    }
}
impl<T> Sizer<T, true>{
    fn size_of() -> usize {
        std::mem::size_of::<T>()
    }
    fn align_of() -> usize {
        std::mem::align_of::<T>()
    }
}

macro_rules! type_prop_array {
    ( [$($T:tt),*] ) => {
        [$(
            TypeProps{
                ty: stringify!($T),
                size: Sizer::<$T, {is_sized::<$T>()}>::size_of(),
                align: Sizer::<$T, {is_sized::<$T>()}>::align_of(),
                is_copy: IsCopy::<$T>::IS_COPY,
                is_clone: IsClone::<$T>::IS_CLONE,
                is_sized: is_sized::<$T>(),
                is_default: IsDefault::<$T>::IS_DEFAULT,
            }
        ),*]
    };
}

fn link_options(tt: &TinyTemplate, out: &mut impl std::io::Write) -> Result<()> {
    let type_props = type_prop_array!([i32, u64]);

    for props in type_props.iter() {
        let def = tt.render("std_option", props)?;
        out.write(def.as_bytes())?;
    }

    Ok(())
}
