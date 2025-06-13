use starlane_space::status::StatusResult;
use crate::backend::Backend;
use crate::backend::call::Call;



#[derive(Debug)]
pub enum Kind {

}


pub trait Handler {
    type Backend: Backend;
    /*
    async fn handle(& self, request: Call<Self::Backend::Method> )  -> Self::Backend::Result;
    
     */
}