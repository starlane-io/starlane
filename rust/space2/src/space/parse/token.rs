use crate::space::parse::nom::Input;
use crate::space::parse::point::PointSeg;
use crate::space::parse::vars::Variable;
use crate::space::point::RouteSeg;

pub struct Token<'a,I> where I:Input+'a {
   input: I,
   kind: TokenKind
}

pub enum TokenKind {
    Comment,
    Point(PointToken),
}


enum PointToken{
   Var(Variable),
   RouteSegPart(RouteSeg),
   SegPart(PointSeg),
   FileRoot,
   FileSegPart,
   FilePart
}