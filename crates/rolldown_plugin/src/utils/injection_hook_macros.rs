#[macro_export]
macro_rules! define_injection_hooks {
  ($( $injection_name:ident ),*) => {
    $(
      pub async fn $injection_name(
        &self,
        args: HookInjectionArgs<'_>,
        mut $injection_name: String,
      ) -> Result<Option<String>> {
        for (plugin, ctx) in &self.plugins {
          if let Some(r) = plugin.$injection_name(ctx, &args).await? {
            $injection_name.push('\n');
            $injection_name.push_str(r.as_str());
          }
        }
        if $injection_name.is_empty() {
          return Ok(None);
        }
        Ok(Some($injection_name))
      }
    )*
  };
}

#[macro_export]
macro_rules! define_injection_hooks_trait {
  ($( $injection_name:ident ),*) => {
    $(
      async fn $injection_name(
        &self,
        _ctx: &SharedPluginContext,
        _args: &HookInjectionArgs,
      ) -> HookInjectionOutputReturn {
        Ok(None)
      }
    )*
  };
}
