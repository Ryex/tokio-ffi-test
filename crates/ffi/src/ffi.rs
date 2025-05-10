
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub struct CxxAny(generated::ZngurCppOpaqueOwnedObject);

unsafe impl Send for CxxAny {}

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




