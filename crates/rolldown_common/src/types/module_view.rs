use crate::{CssView, ecmascript::ecma_view::EcmaView};

pub enum ModuleView {
  Ecma(Box<EcmaView>),
  Css(CssView),
}
