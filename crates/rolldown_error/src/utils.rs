// use arcstr::ArcStr;
// use ariadne::{Label, Span};
// use rustc_hash::FxHashMap;
//
// use crate::diagnostic::{DiagnosticFileId, RolldownLabelSpan};
//
// pub fn is_context_too_long(
//   label: &Label<RolldownLabelSpan>,
//   files: &FxHashMap<DiagnosticFileId, ArcStr>,
// ) -> bool {
//   let span = label.span();
//   let source_id = span.source();
//   let source = files.get(source_id).expect("should have file");
//   if source.len() < 600 {
//     return false;
//   }
//   todo!()
// }
