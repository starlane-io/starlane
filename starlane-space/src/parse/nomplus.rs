use nom::{AsChar, InputTakeAtPosition};
use nom::error::ErrorKind;
use crate::parse::Res;
use crate::parse::util::Span;

pub(crate) fn lowercase1<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item.is_alpha() && char_item.is_lowercase())
        },
        ErrorKind::Alpha,
    )
}