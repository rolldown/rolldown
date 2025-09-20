use crate::output::Output;
use std::path::PathBuf;

use anyhow::Result;

mod checks;
mod hook_usage;
mod oxc_runtime_helper;
mod runtime_helper;
pub use checks::CheckOptionsGenerator;
pub use hook_usage::HookUsageGenerator;
pub use oxc_runtime_helper::OxcRuntimeHelperGenerator;
pub use runtime_helper::RuntimeHelperGenerator;

/// Trait to define a generator.
pub trait Generator: Runner {
  fn generate(&self, _ctx: &Context) -> Output {
    unimplemented!()
  }

  fn generate_many(&self, ctx: &Context) -> Result<Vec<Output>> {
    Ok(vec![self.generate(ctx)])
  }
}

/// Macro to implement [`Runner`] for a [`Generator`].
///
/// Must be used on every [`Generator`].
///
/// # Example
/// ```ignore
/// use gen_options::define_generator;
/// struct AssertLayouts;
/// define_generator!(AssertLayouts);
/// ```
#[macro_export]
macro_rules! define_generator {
    ($ident:ident $($lifetime:lifetime)?) => {
        const _: () = {
            use anyhow::Result;
            use $crate::{
                output::Output,
                generators::{ Runner, Context }
            };


            impl $($lifetime)? Runner for $ident $($lifetime)? {
                fn name(&self) -> &'static str {
                    stringify!($ident)
                }

                fn file_path(&self) -> &'static str {
                    file!()
                }

                fn run(&self, ctx: &Context) -> Result<Vec<Output>> {
                    self.generate_many(ctx)
                }
            }
        };
    };
}

/// Runner trait.
///
/// This is the super-trait of [`Derive`] and [`Generator`].
///
/// [`Generator`]: crate::Generator
pub trait Runner {
  fn name(&self) -> &'static str;

  fn file_path(&self) -> &'static str;

  fn run(&self, ctx: &Context) -> Result<Vec<Output>>;
}

pub struct Context {
  pub workspace_root: PathBuf,
}
