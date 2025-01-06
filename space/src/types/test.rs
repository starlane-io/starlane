#![cfg(test)]



pub mod parse {
    use std::str::FromStr;
    use starlane_space::parse::unwrap_block;
    use crate::err::{ParseErrs, PrintErr};
    use crate::parse::{camel_case, from_camel, CamelCase};
    use crate::parse::model::{BlockKind, NestedBlockKind};
    use crate::parse::util::{new_span, result};
    use crate::types::class::{Class, ClassDiscriminant, ClassParsers};
    use crate::types::private::{Generic, Parsers};
    use crate::types::Schema;
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

        let class = result(Class::parse_outer(i)).unwrap();


        assert_eq!(Class::Database, class);


    }



    #[test]
    pub fn test_class_discriminant() {
        let s = "<Service<Database>>";
        let i = new_span(s);

        let parser = Class::parser();
    }


    #[test]
    pub fn test_class_variant() {
        let s = "<Service<Database>>";



        let i = new_span(s);
        match result(Class::parse_outer(i))  {
            Ok(_) => {}
            Err(err) => {
                err.print();
                panic!("test_class_variant failed: {}", err)
            }
        }
    }

    #[test]
    pub fn test_schema() {
        let inner = "Text";
        let s = format!("[{}]", inner);
        let i = new_span(s.as_str());
        let schema = result(Schema::parse_outer(i)).unwrap();
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