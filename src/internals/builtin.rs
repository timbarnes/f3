/// Interpreter for builtins
///
/// Set up a table of builtin functions, with names and code

#[allow(dead_code)]
use crate::runtime::ForthRuntime;

// The mechanism for storing and calling function pointers

// The internal format for builtins: a name, code pointer, and documentation string for use by SEE
pub struct BuiltInFn {
    pub name: String,
    pub code: fn(&mut ForthRuntime), // Function pointer
    pub doc: String,
}

impl BuiltInFn {
    pub fn new(name: String, code: fn(&mut ForthRuntime), doc: String) -> BuiltInFn {
        BuiltInFn { name, code, doc }
    }

    pub fn call(&self, rt: &mut ForthRuntime) {
        (self.code)(rt);
    }
}
