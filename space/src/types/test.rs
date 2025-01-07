#![cfg(test)]



pub mod parse {
    use std::str::FromStr;
    use starlane_space::parse::unwrap_block;
    use crate::parse::{camel_case, from_camel, CamelCase};
    use crate::parse::model::{BlockKind, NestedBlockKind};
    use crate::parse::util::{new_span, result};
    use crate::types::class::{Class, ClassDiscriminant};
    use crate::types::class::service::Service;
    use crate::types::parse::{TypeParsers, PrimitiveParser};
    use crate::types::private::{Generic};
    use crate::types::{ClassExt, Schema};


    #[test]
    pub fn test_parse_exact() {
        let inner = "Database@uberscott.io:postgres:1.3.5";
        let outer = NestedBlockKind::Angle.wrap(&inner);
        let input = new_span(outer.as_str());
        let ext = result(<ClassExt as BlockParser>::block().unwrap(ClassExt::parse)(input)).unwrap();
        println!("from -> {}", outer );
        println!("ext -> {}", ext );
    }


    #[test]
    pub fn test_from_camel() {
        #[derive(Eq, PartialEq, Debug)]
        struct Blah(CamelCase);

        impl From<CamelCase> for Blah {
            fn from(camel: CamelCase) -> Self {
                Blah(camel)
            }
        }

        let s = "MyCamelCase";
        let i = new_span(s);
        let blah: Blah = result(from_camel(i)).unwrap();
        assert_eq!(blah.0.as_str(), s);
    }

    #[test]
    pub fn test_class() {
        let inner = "Database";
        let s = format!("<{}>", inner).to_string();
        let i = new_span(s.as_str());

        let res = result(unwrap_block(BlockKind::Nested(NestedBlockKind::Angle), camel_case)(i.clone())).unwrap();
        assert_eq!(res.as_str(), inner);


        let (next, disc) = ClassParsers::discriminant(new_span(inner)).unwrap();

        assert_eq!(ClassDiscriminant::Database, disc);

        assert!(!ClassParsers::peek_variant(next));

        let class = result(Class::parse(new_span(inner))).unwrap();

        assert_eq!(Class::Database, class);

        let class = result(Class::outer(i)).unwrap();


        assert_eq!(Class::Database, class);


    }



    #[test]
    pub fn test_class_discriminant() {
        let s = "<Service<Database>>";
        let i = new_span(s);

        let parser = Class::parse(i).unwrap();
    }


    #[test]
    pub fn test_class_variant() {
        let inner = "Service<Database>";
        let outer  = format!("<{}>",inner).to_string();

        let segment = result(ClassParsers::segment(new_span(inner))).unwrap();
        assert_eq!(segment.as_str(), "Service");

        let (next,discriminant) = ClassParsers::discriminant(new_span(inner)).unwrap();
        assert_eq!(ClassDiscriminant::Service, discriminant);
        assert_eq!(next.to_string().as_str(), "<Database>");
        assert!(ClassParsers::peek_variant(next.clone()));

        let variant = result(ClassParsers::block(ClassParsers::segment)(next)).unwrap();
        assert_eq!(variant.to_string().as_str(), "Database");

        let class = ClassParsers::create(discriminant, variant).unwrap();

        assert_eq!(class,Class::Service(Service::Database));

        let class = result(Class::outer(new_span(outer.as_str()))).unwrap();

        assert_eq!(Class::Service(Service::Database),class)

    }

    #[test]
    pub fn test_schema() {
        let inner = "Text";
        let s = format!("[{}]", inner);
        let i = new_span(s.as_str());
        let schema = result(Schema::outer(i)).unwrap();
        assert_eq!(schema.to_string().as_str(), inner);
    }

    #[test]
    pub fn class_from_camel() {
        let camel = CamelCase::from_str("Database").unwrap();
        let class = Class::from(camel);

        assert_eq!(class, Class::Database);
    }

    #[test]
    pub fn test_class_ext() {
        /// test [Class:_Ext]
        let camel = CamelCase::from_str("Zophis").unwrap();
        let class = Class::from(camel.clone());

        assert_eq!(class, Class::_Ext(camel));
    }
}