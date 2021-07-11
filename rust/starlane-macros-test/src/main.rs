use starlane_macros::resources;


fn main() {
    println!("Hello, world!");
}

pub enum StarKind{
    Central,
    Space,
    App
}

pub struct Specific{

}


resources! {


    #[resource(parents(Root))]
    #[resource(stars(Space))]
    #[resource(prefix="spc")]
    pub struct Space{

    }

    #[resource(parents(Space))]
    #[resource(stars(Space))]
    #[resource(prefix="sub")]
    pub struct SubSpace{

    }

    #[resource(parents(SubSpace))]
    #[resource(stars(App))]
    #[resource(prefix="app")]
    pub struct App{

    }

    #[resource(parents(SubSpace,App))]
    #[resource(stars(Space,App))]
    #[resource(prefix="db")]
    pub struct Database{
    }


    pub enum DatabaseKind{
        Native,
        External(Specific)
    }

}



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
