use crate::ecmascript::format::utils::external_module::ExternalModules;
use crate::ecmascript::format::utils::namespace::generate_identifier;
pub use crate::ecmascript::format::utils::wrapper::render_wrapper_function as render_iife;
use crate::types::generator::GenerateContext;
use rolldown_common::OutputExports;
use rolldown_error::DiagnosableResult;

pub fn render_iife_factory(
  ctx: &mut GenerateContext<'_>,
  export_mode: &OutputExports,
  has_export: bool,
  args: &ExternalModules,
) -> DiagnosableResult<(String, String)> {
  let (definition, assignment) = generate_identifier(ctx, export_mode, has_export, "this")?;
  let named_export = matches!(&export_mode, OutputExports::Named);
  let export_invoker = if has_export && named_export {
    if ctx.options.extend {
      // If using `output.extend`, the first caller argument should be `name = name || {}`,
      // then the result will be assigned to `name`.
      Some(assignment.as_str())
    } else {
      // If not using `output.extend`, the first caller argument should be `{}`,
      // then the result will be assigned to `exports`.
      Some("{}")
    }
  } else {
    // If there is no export or not using named export,
    // there shouldn't be an argument shouldn't be related to the export.
    None
  };
  let caller = format!("({})", args.as_iife(ctx, export_invoker.unwrap_or_default()));
  let assigner = if (ctx.options.extend && named_export) || !has_export || assignment.is_empty() {
    // If facing following situations, there shouldn't an assignment for the wrapper function:
    // - Using `output.extend` and named export.
    // - No export.
    // - the `assignment` is empty.
    String::new()
  } else {
    format!("{assignment} = ")
  };
  let invoker = format!("{definition}{assigner}");
  Ok((invoker, caller))
}
