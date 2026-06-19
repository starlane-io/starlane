use crate::exports::starlane::hyperspace::filter_api::Guest;
use email_address::EmailAddress;

wit_bindgen::generate!({
    path: "../../wit",
    world: "filter",
});

pub use Status::Panic;
use crate::starlane::hyperspace::space::Status;

struct PluginComponent;

impl Guest for PluginComponent {

    fn filter(input: String) -> Result<String, String> {
        match EmailAddress::is_valid(&input) {
            true => {
                crate::starlane::hyperspace::status_api::update(Status::Ready);
                Ok(input)
            },
            false => {
                crate::starlane::hyperspace::status_api::update(Status::Panic);
                Err(format!("Invalid email address: {}", input))
            },
        }
    }
}


export!(PluginComponent);

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
