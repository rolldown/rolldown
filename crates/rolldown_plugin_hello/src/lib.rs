use rolldown::{HookNoopReturn, Plugin, PluginContext};
use rolldown_fs::FileSystem;
use std::borrow::Cow;

#[derive(Debug)]
pub struct HelloPlugin;

#[async_trait::async_trait]
impl<T: FileSystem + 'static + Default> Plugin<T> for HelloPlugin {
  fn name(&self) -> Cow<'static, str> {
    "hello".into()
  }

  #[allow(clippy::print_stdout)]
  async fn build_start(&self, _ctx: &PluginContext<T>) -> HookNoopReturn {
    println!("hello");
    Ok(())
  }
}
