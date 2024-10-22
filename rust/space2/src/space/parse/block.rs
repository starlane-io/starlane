use thiserror_no_std::Error;
use crate::space::parse::ctx::{InputCtx, PrimCtx, ToInputCtx};
use crate::space::parse::nomplus::Input;

#[derive(Error,Debug, Copy, Clone, Eq, PartialEq)]
pub enum BlockKind {
    #[error("nested block")]
    Nested(#[from] NestedBlockKind),
    #[error("terminated")]
    Terminated(#[from] TerminatedBlockKind),
    #[error("delimited")]
    Delimited(#[from] DelimitedBlockKind),
    #[error("partial")]
    Partial,
}

impl ToInputCtx for BlockKind{
    fn to(self) -> impl Fn()->InputCtx
    {
        move || InputCtx::Block(self)
    }
}


#[derive(Error,Debug, Copy, Clone,  Eq, PartialEq)]
pub enum TerminatedBlockKind {
    #[error("semicolon")]
    Semicolon,
}
impl ToInputCtx for TerminatedBlockKind{
    fn to(self) -> impl Fn()->InputCtx
    {
        move || InputCtx::Block(BlockKind::Terminated(self))
    }
}

impl TerminatedBlockKind {
    pub fn tag(&self) -> &'static str {
        match self {
            TerminatedBlockKind::Semicolon => ";",
        }
    }

    pub fn as_char(&self) -> char {
        match self {
            TerminatedBlockKind::Semicolon => ';',
        }
    }
}

#[derive(
    Debug, Copy, Clone, Error, Eq, PartialEq,
)]
pub enum DelimitedBlockKind {
    #[error("single quotes")]
    SingleQuotes,
    #[error("double quotes")]
    DoubleQuotes,
}

impl ToInputCtx for DelimitedBlockKind{
    fn to(self) -> impl Fn()->InputCtx
    {
        move || InputCtx::Block(BlockKind::Delimited(self))
    }
}
impl DelimitedBlockKind {
    pub fn delim(&self) -> &'static str {
        match self {
            DelimitedBlockKind::SingleQuotes => "'",
            DelimitedBlockKind::DoubleQuotes => "\"",
        }
    }

    pub fn escaped(&self) -> &'static str {
        match self {
            DelimitedBlockKind::SingleQuotes => "\'",
            DelimitedBlockKind::DoubleQuotes => "\"",
        }
    }

    pub fn context(&self) -> &'static str {
        match self {
            DelimitedBlockKind::SingleQuotes => "single:quotes:block",
            DelimitedBlockKind::DoubleQuotes => "double:quotes:block",
        }
    }

    pub fn missing_close_context(&self) -> &'static str {
        match self {
            DelimitedBlockKind::SingleQuotes => "single:quotes:block:missing-close",
            DelimitedBlockKind::DoubleQuotes => "double:quotes:block:missing-close",
        }
    }
}

#[derive(Error,Debug, Copy, Clone,  Eq, PartialEq)]
pub enum NestedBlockKind {
    #[error("curly")]
    Curly,
    #[error("parenthesis")]
    Parens,
    #[error("square")]
    Square,
    #[error("angle")]
    Angle,
}

impl ToInputCtx for NestedBlockKind{
    fn to(self) -> impl Fn()->InputCtx
    {
        move || InputCtx::Block(BlockKind::Nested(self))
    }
}

impl NestedBlockKind {
    pub fn is_block_terminator(c: char) -> bool {
        match c {
            '}' => true,
            ')' => true,
            ']' => true,
            '>' => true,
            _ => false,
        }
    }

    pub fn context(&self) -> &'static str {
        match self {
            NestedBlockKind::Curly => "block:{}",
            NestedBlockKind::Parens => "block:()",
            NestedBlockKind::Square => "block:[]",
            NestedBlockKind::Angle => "block:<>",
        }
    }



    pub fn open_context(&self) -> &'static str {
        match self {
            NestedBlockKind::Curly => "block:open:{",
            NestedBlockKind::Parens => "block:open:(",
            NestedBlockKind::Square => "block:open:[",
            NestedBlockKind::Angle => "block:open:<",
        }
    }

    pub fn close_context(&self) -> &'static str {
        match self {
            NestedBlockKind::Curly => "block:close:}",
            NestedBlockKind::Parens => "block:close:)",
            NestedBlockKind::Square => "block:close:]",
            NestedBlockKind::Angle => "block:close:>",
        }
    }

    pub fn unpaired_closing_scope(&self) -> &'static str {
        match self {
            NestedBlockKind::Curly => "block:close-before-open:}",
            NestedBlockKind::Parens => "block:close-before-open:)",
            NestedBlockKind::Square => "block:close-before-open:]",
            NestedBlockKind::Angle => "block:close-before-open:>",
        }
    }

    pub fn open(&self) -> &'static str {
        match self {
            NestedBlockKind::Curly => "{",
            NestedBlockKind::Parens => "(",
            NestedBlockKind::Square => "[",
            NestedBlockKind::Angle => "<",
        }
    }

    pub fn close(&self) -> &'static str {
        match self {
            NestedBlockKind::Curly => "}",
            NestedBlockKind::Parens => ")",
            NestedBlockKind::Square => "]",
            NestedBlockKind::Angle => ">",
        }
    }

    pub fn open_as_char(&self) -> char {
        match self {
            NestedBlockKind::Curly => '{',
            NestedBlockKind::Parens => '(',
            NestedBlockKind::Square => '[',
            NestedBlockKind::Angle => '<',
        }
    }

    pub fn close_as_char(&self) -> char {
        match self {
            NestedBlockKind::Curly => '}',
            NestedBlockKind::Parens => ')',
            NestedBlockKind::Square => ']',
            NestedBlockKind::Angle => '>',
        }
    }
}
