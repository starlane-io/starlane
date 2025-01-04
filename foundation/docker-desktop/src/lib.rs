

pub use starlane_base as base;

pub trait Foundation: base::Foundation {}

mod concrete {}

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {}
}
