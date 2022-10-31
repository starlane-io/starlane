use handlebars::RenderError;

use mechtron::err::MechErr;

#[derive(cosmic_macros_primitive::MechErr)]
pub struct MyErr {
    pub message: String
}

impl From<RenderError> for MyErr {
    fn from(e: RenderError) -> Self {
        Self {
            message: e.to_string()
        }
    }
}