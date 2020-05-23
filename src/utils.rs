use std::any::Any;
use std::fmt::Display;

pub fn maybe_error<T: Any, U: Display>(result: Result<T, U>) {
    match result {
        Ok(_) => (),
        Err(e) => eprintln!("{}", e),
    }
}
