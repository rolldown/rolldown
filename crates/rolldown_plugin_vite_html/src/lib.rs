mod html;

use std::{borrow::Cow, rc::Rc};

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

  #[expect(unused_variables, unused_assignments)]
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

    let dom = html::parser::parse_html(args.code);
    let mut s = string_wizard::MagicString::new(args.code);

    // TODO: Extract into a function
    let mut stack = vec![dom.document];
    while let Some(node) = stack.pop() {
      match &node.data {
        html::sink::NodeData::Element { name, attrs, .. } => {
          if &**name == "script" {
            let mut src = None;
            let mut loc = None;
            let mut is_async = false;
            let mut is_module = false;
            let mut is_ignored = false;
            for attr in attrs.borrow().iter() {
              match &*attr.name {
                "src" => {
                  if src.is_none() {
                    loc = Some(attr.span);
                    src = Some(attr.value.clone());
                  }
                }
                "type" if attr.value == "module" => {
                  is_module = true;
                }
                "async" => {
                  is_async = true;
                }
                "vite-ignore" => {
                  is_ignored = true;
                  s.remove(attr.span.start, attr.span.end);
                }
                _ => {}
              }
            }
            if !is_ignored {
              todo!()
            }
          }
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
