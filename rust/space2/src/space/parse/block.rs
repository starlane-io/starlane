use thiserror_no_std::Error;

#[derive(Error,Debug, Copy, Clone, Eq, PartialEq)]
pub enum BlockKind {
    #[error("nexted block")]
    Nested(#[from] NestedBlockKind),
    #[error("terminated")]
    Terminated(#[from] TerminatedBlockKind),
    #[error("delimited")]
    Delimited(#[from] DelimitedBlockKind),
    #[error("partial")]
    Partial,
}

#[derive(Error,Debug, Copy, Clone,  Eq, PartialEq)]
pub enum TerminatedBlockKind {
    #[error("semicolon")]
    Semicolon,
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

    pub fn error_message<I: Span>(span: &I, context: &str) -> Result<&'static str, ()> {
        if Self::Curly.open_context() == context {
            Ok("expecting '{' (open scope block)")
        } else if Self::Parens.open_context() == context {
            Ok("expecting '(' (open scope block)")
        } else if Self::Angle.open_context() == context {
            Ok("expecting '<' (open scope block)")
        } else if Self::Square.open_context() == context {
            Ok("expecting '[' (open scope block)")
        } else if Self::Curly.close_context() == context {
            Ok("expecting '}' (close scope block)")
        } else if Self::Parens.close_context() == context {
            Ok("expecting ')' (close scope block)")
        } else if Self::Angle.close_context() == context {
            Ok("expecting '>' (close scope block)")
        } else if Self::Square.close_context() == context {
            Ok("expecting ']' (close scope block)")
        } else if Self::Curly.unpaired_closing_scope() == context {
            Ok("closing scope without an opening scope")
        } else if Self::Parens.unpaired_closing_scope() == context {
            Ok("closing scope without an opening scope")
        } else if Self::Angle.unpaired_closing_scope() == context {
            Ok("closing scope without an opening scope")
        } else if Self::Square.unpaired_closing_scope() == context {
            Ok("closing scope without an opening scope")
        } else {
            Err(())
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
