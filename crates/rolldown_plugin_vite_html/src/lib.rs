use std::{borrow::Cow, rc::Rc};

use html5ever::{ParseOpts, parse_document, tendril::TendrilSink as _};
use markup5ever_rcdom as rcdom;
use rolldown_plugin::{HookUsage, Plugin};
use rolldown_utils::pattern_filter::normalize_path;
use sugar_path::SugarPath as _;

#[derive(Debug, Default)]
pub struct ViteHtmlPlugin;

impl Plugin for ViteHtmlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-html")
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    HookUsage::Transform | HookUsage::GenerateBundle
  }

  #[expect(unused_variables)]
  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !args.id.ends_with(".html") {
      return Ok(None);
    }

    let path = args.id.relative(ctx.cwd());
    let relative_url_path = normalize_path(&path.to_string_lossy());

    let dom = parse_document(rcdom::RcDom::default(), ParseOpts::default()).one(args.code.as_str());

    // TODO: Extract into a function
    let mut stack = vec![dom.document];
    while let Some(node) = stack.pop() {
      match &node.data {
        rcdom::NodeData::Element { name, .. } if &*name.local == "script" => {
          todo!()
        }
        _ => {}
      }
      for child in node.children.borrow().iter() {
        stack.push(Rc::clone(child));
      }
    }

    todo!()
  }

  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    _args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    todo!()
  }
}
