use crate::{CssView, ecmascript::ecma_view::EcmaView};

pub enum ModuleView {
  Ecma(EcmaView),
  Css(CssView),
}
