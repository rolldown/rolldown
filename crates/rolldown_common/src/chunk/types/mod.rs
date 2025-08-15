pub mod chunk_reason_type;
pub mod cross_chunk_import_item;
pub mod module_group;
pub mod preliminary_filename;

pub struct AddonRenderContext<'code> {
  pub hashbang: Option<&'code str>,
  pub banner: Option<&'code str>,
  pub intro: Option<&'code str>,
  pub outro: Option<&'code str>,
  pub footer: Option<&'code str>,
  pub directives: &'code [&'code str],
}
