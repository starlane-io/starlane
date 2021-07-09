use starlane_macros::resources;


fn main() {
    println!("Hello, world!");
}



resources! {

    #[resource(parents())]
    #[resource(stars(Central))]
    pub struct Root{

    }

    #[resource(parents(Root))]
    #[resource(stars(Space))]
    pub struct Space{

    }

    #[resource(parents(Space))]
    #[resource(stars(Space))]
    pub struct SubSpace{

    }

    #[resource(parents(SubSpace))]
    #[resource(stars(App))]
    pub struct App{

    }

    #[resource(parents(SubSpace,App))]
    #[resource(stars(Space,App))]
    pub struct FileSystem{
    }

    #[resource(parents(SubSpace,App))]
    #[resource(stars(Space,App))]
    pub struct Database{
    }


    pub enum DatabaseKind{
        Relational(Specific)
    }

}


pub trait Kind{

}

pub enum Something{}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
