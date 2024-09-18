use crate::{ecmascript::ecma_view::EcmaView, CssView};

pub enum ModuleView {
  Ecma(EcmaView),
  Css(CssView),
}
