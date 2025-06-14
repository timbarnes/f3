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
}

/////////////////////////////////////////////
/// TESTS
/// 
#[cfg(test)]

mod tests {
    use super::*;
    use crate::runtime::ForthRuntime;

    #[test]

    fn test_builtin_fn_creation() {
        let mut rt = ForthRuntime::new();
        fn num_fn(rt: &mut ForthRuntime) {
            rt.kernel.push(44);
        }
        fn get_val(rt: &mut ForthRuntime) -> i64 {
            let builtin_fn = BuiltInFn::new("test".to_string(),
                num_fn,
                "This is a test function".to_string()
            ); 
            (builtin_fn.code)(rt);      // Call the function pointer
            rt.kernel.pop()             // Get the value pushed by the function
        }
        let name = "test".to_string();
        let doc = "This is a test function".to_string();

        let builtin_fn = BuiltInFn::new(name, num_fn, doc);
        assert_eq!(builtin_fn.name, "test");
        assert_eq!(builtin_fn.doc, "This is a test function");
        assert_eq!(builtin_fn.code as usize, num_fn as usize);
        assert_eq!(get_val(&mut rt), 44);
    }
}