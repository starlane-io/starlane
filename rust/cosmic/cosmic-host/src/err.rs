use std::fmt::Display;

#[derive(Clone,Debug)]
pub struct Err {
    message: String,
}

impl Err {
    pub fn to_string(&self) -> String {
        self.message.clone()
    }
}
/*
impl ToString for Err {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

 */




impl <T> From<T> for Err where T: ToString {
    fn from(value: T) -> Self {
        Self {
            message: value.to_string()
        }
    }
}


impl Err {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}






/*
impl From<String> for Err {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<WasiStateCreationError> for Err {
    fn from(value: WasiStateCreationError) -> Self {
        Err( )
    }
}
impl From<WasiRuntimeError> for Err {
    fn from(value: WasiRuntimeError) -> Self {
       Err {
           message: value.to_string()
       }
    }
}
impl From<WasiStateCreationError> for Err {
    fn from(value: WasiStateCreationError) -> Self {
        Self {
            message: value.to_string()
        }
    }
}

impl From<io::Error> for Err {
    fn from(err: io::Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl ToString for Err {
    fn to_string(&self) -> String {
        todo!()
    }
}


 */