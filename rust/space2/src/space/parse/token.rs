use crate::space::parse::nomplus::Input;
use crate::space::parse::point::PointSeg;
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
//   Var(Variable),
   RouteSegPart(RouteSeg),
   SegPart(PointSeg),
   FileRoot,
   FileSegPart,
   FilePart
}