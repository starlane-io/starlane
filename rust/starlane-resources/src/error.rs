use std::fmt::Display;

#[derive(Debug,Clone)]
pub struct Error {
    pub message: String
}

impl <T:Display> From<T> for Error{
    fn from(t: T) -> Self {
        Error{message:format!("{}",t)}
    }
}

impl ToString for Error {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}
