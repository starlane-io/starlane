use core::ops::Range;
use core::cmp::min;
use nom::AsChar;
use crate::lib::std::string::String;
use crate::lib::std::string::ToString;
use crate::lib::std::str::FromStr;
use crate::lib::std::ops::Deref;
use starlane_primitive_macros::Case;
use crate::space::parse::ctx::CaseCtx;
use crate::space::parse::nomplus::err::ParseErr;

pub trait Case {
    fn validate<S>( s: &S ) -> Result<(),ParseErr> where S: AsRef<str> + ?Sized;
}

impl Case for SkewerCase {
    fn validate<S>(string: &S) -> Result<(), ParseErr>
    where
        S: AsRef<str> + ?Sized
    {
        for (index, c) in string.as_ref().char_indices() {
            if (index == 0)
            {
                if (!c.is_alpha() || !c.is_lowercase()) {
                    let range = Range::from(0..1);
                    let err = ParseErr::new(CaseCtx::SkewerCase, "skewer case must start with a lowercase alpha character", range);
                    return Err(err);
                }
            } else {
                if (c.is_alpha() && !c.is_lowercase()) || !(c.is_digit(10) || c == '-') {
                    let range = Range::from(index - 1..index);
                    let err = ParseErr::new(CaseCtx::SkewerCase, "valid skewer case characters are lowercase alpha, digits 0-9 and dash '-'", range);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}

impl Case for VarCase{

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where
        S: AsRef<str> + ?Sized{
        for (index,c) in string.as_ref().char_indices() {
            if( index == 0 )
            {
                if (!c.is_alpha() || !c.is_lowercase()) {
                    let range = Range::from( 0..1);
                    let err = ParseErr::new(CaseCtx::VarCase,"VarCase must start with a lowercase alpha character", range);
                    return Err(err);
                }
            } else {
                if (c.is_alpha() && !c.is_lowercase()) || !(c.is_digit(10)||c == '_') {
                    let range = Range::from(index-1..index );
                    let err = ParseErr::new(CaseCtx::VarCase,"valid VarCase case characters are lowercase alpha, digits 0-9 and underscore '_'", range);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}

impl Case for  DomainCase{

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where
        S: AsRef<str> + ?Sized{
        for (index,c) in string.as_ref().char_indices() {
            if( index == 0 )
            {
                if (!c.is_alpha() || !c.is_lowercase()) {
                    let range = Range::from( 0..1);
                    let err = ParseErr::new(CaseCtx::DomainCase,"DomainCase must start with a lowercase alpha character", range);
                    return Err(err);
                }
            } else {
                if (c.is_alpha() && !c.is_lowercase()) || !(c.is_digit(10)||c == '-'||c == '.') {
                    let range = Range::from(index-1..index );
                    let err = ParseErr::new(CaseCtx::DomainCase,"valid DomainCase case characters are lowercase alpha, digits 0-9 and dash '-' and dot '.'", range);
                    return Err(err);
                }
            }
        }
        Ok(())
    }

}

impl Case for CamelCase{

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where S: AsRef<str>+?Sized{
        for (index,c) in string.as_ref().char_indices() {
            if( index == 0 )
            {
                if (!c.is_alpha() || !c.is_uppercase()) {
                    let range = Range::from( 0..1);
                    let err = ParseErr::new(CaseCtx::CamelCase,"CamelCase must start with an uppercase alpha character", range);
                    return Err(err);
                }
            } else {
                if (c.is_alpha() && !c.is_lowercase()) || !(c.is_digit(10)||c == '-'||c == '.') {
                    let range = Range::from(index-1..index );
                    let err = ParseErr::new(CaseCtx::CamelCase,"valid CamelCase characters are mixed case alpha, digits 0-9", range);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}

impl Case for FileCase{

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where S: AsRef<str> + ?Sized{
        for (index,c) in string.as_ref().char_indices() {
                if !(c.is_alpha() || c.is_digit(10)||c == '-'||c == '.'|| c=='_') {
                    let start = min(0,index-1);
                    let range = Range::from(start..index );
                    let err = ParseErr::new(CaseCtx::FileCase,"valid FileCase case characters are lowercase alpha, digits 0-9 and dash '-', dot '.' and underscore '_'", range);
                    return Err(err);
                }
        }
        Ok(())
    }
}

impl Case for DirCase {

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where S: AsRef<str>+?Sized{
        for (index,c) in string.as_ref().char_indices() {
            if !(c.is_alpha() || c.is_digit(10)||c == '-'||c == '.'|| c=='_') {
                let start = min(0,index-1);
                let range = Range::from(start..index );
                let err = ParseErr::new(CaseCtx::DirCase,"valid DirCase case characters are lowercase alpha, digits 0-9 and dash '-', dot '.' and underscore '_' and must terminate with a '/'", range);
                return Err(err);
            }
        }
        Ok(())
    }
}

#[derive(Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct SkewerCase(pub(crate) String);

#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct VarCase(pub(crate) String);

#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct DomainCase(pub(crate) String);

#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct CamelCase(pub(crate) String);

#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct FileCase(pub(crate) String);

#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct DirCase(pub(crate) String);