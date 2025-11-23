
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub struct CxxAny(generated::ZngurCppOpaqueOwnedObject);
pub struct CxxException(generated::ZngurCppOpaqueOwnedObject);

unsafe impl Send for CxxAny {}
unsafe impl Send for CxxException {}

impl std::fmt::Debug for CxxException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CxxException {{ msg: {:?} }}", self.what())
    }
}

impl std::fmt::Display for CxxException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.what())
    }
}

impl std::fmt::Debug for CxxAny {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_debug(f)
    }
}

/// ffi helper for iterator size hints
pub trait SizeHintExt {
    fn lower(&self) -> usize;

    fn upper (&self) -> Option<usize>;
}
 
impl SizeHintExt for (usize, Option<usize>) {
    fn lower(&self) -> usize {
        self.0
    }
    fn upper (&self) -> Option<usize> {
        self.1
    }
}


