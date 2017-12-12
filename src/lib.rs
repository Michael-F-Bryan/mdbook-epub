extern crate epub_builder;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate mdbook;
extern crate serde_json;


use mdbook::renderer::RenderContext;

pub fn generate(ctx: &RenderContext) -> Result<(), RenderError> {
    unimplemented!()
}


#[derive(Debug, Clone, PartialEq, Fail)]
#[fail(display = "Rendering Failed")]
pub struct RenderError {}
