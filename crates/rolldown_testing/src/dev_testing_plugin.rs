use rolldown::plugin::{HookUsage, Plugin};

pub type DevTestingPluginReceiver = tokio::sync::mpsc::UnboundedReceiver<()>;
pub type DevTestingPluginSender = tokio::sync::mpsc::UnboundedSender<()>;

#[derive(Debug)]
pub struct DevTestingPlugin {
  pub receiver: DevTestingPluginReceiver,
}

impl DevTestingPlugin {
  pub fn new() -> (Self, DevTestingPluginSender) {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    (Self { receiver }, sender)
  }
}

impl Plugin for DevTestingPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "dev-testing".into()
  }

  fn register_hook_usage(&self) -> rolldown::plugin::HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: rolldown::plugin::SharedTransformPluginContext,
    args: &rolldown::plugin::HookTransformArgs<'_>,
  ) -> rolldown::plugin::HookTransformReturn {
    // Simulate a long-running transform
    if args.code.contains("// @delay-transform") {
      tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Ok(None)
  }
}
