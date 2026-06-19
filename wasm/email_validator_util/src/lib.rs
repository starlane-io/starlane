use crate::exports::starlane::hyperspace::filter_api::Guest;
use email_address::EmailAddress;

wit_bindgen::generate!({
    path: "../../wit",
    world: "filter",
});

struct PluginComponent;

impl Guest for PluginComponent {
    fn filter(input: String) -> Result<String, String> {
        match EmailAddress::is_valid(&input) {
            true => Ok(input),
            false => Err(format!("Invalid email address: {}", input)),
        }
    }
}

export!(PluginComponent);

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
