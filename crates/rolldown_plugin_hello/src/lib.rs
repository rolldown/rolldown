use std::borrow::Cow;

use rolldown_plugin::{HookNoopReturn, Plugin, PluginContext};

#[derive(Debug)]
pub struct HelloPlugin;

#[async_trait::async_trait]
impl Plugin for HelloPlugin {
  fn name(&self) -> Cow<'static, str> {
    "hello".into()
  }

  #[allow(clippy::print_stdout)]
  async fn build_start(&self, _ctx: &mut PluginContext) -> HookNoopReturn {
    println!("hello");
    Ok(())
  }
}
