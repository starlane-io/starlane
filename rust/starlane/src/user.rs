
pub struct User
{
  pub name: String
}

impl User
{
    pub fn new( name: String ) -> Self
    {
        User{
            name: name
        }
    }
}