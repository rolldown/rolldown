use arcstr::ArcStr;
use rolldown_common::file_view::FileView;

pub mod file_generator;

pub fn create_file_view(source: &ArcStr) -> FileView {
  FileView { source: source.clone() }
}
