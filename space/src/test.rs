use starlane_macros::proxy;

#[test]
fn test() {





}



#[proxy(prefix = "Lordie")]
pub trait My {
    fn some_method(&self, limits: u8) -> Result<(),()>;
    async fn gobot(&self, stringy: &str ) -> Result<(),()>;
}
