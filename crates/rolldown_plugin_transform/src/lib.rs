use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, SharedPluginContext,
};
use std::borrow::Cow;

#[derive(Debug)]
pub struct EcmaTransformPlugin {
  pub skip: bool,
}

impl Plugin for EcmaTransformPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("module_preload_polyfill")
  }
  async fn transform(
    &self,
    ctx: &rolldown_plugin::TransformPluginContext<'_>,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let code = args.code;
    todo!()
    // let oxc_source_type = {
    //   let default = pure_esm_js_oxc_source_type();
    //   match parsed_type {
    //     OxcParseType::Js => default,
    //     OxcParseType::Jsx => default.with_jsx(true),
    //     OxcParseType::Ts => default.with_typescript(true),
    //     OxcParseType::Tsx => default.with_typescript(true).with_jsx(true),
    //   }
    // };
    //
    // let source = ArcStr::from(source);
    // let parse_result = EcmaCompiler::parse(stable_id, &source, oxc_source_type);
    //
    // let mut ecma_ast = match parse_result {
    //   Ok(ecma_ast) => ecma_ast,
    //   Err(errs) => {
    //     return Ok(Err(errs));
    //   }
    // };
    // }
  }
}
