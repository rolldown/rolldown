use arcstr::ArcStr;

pub enum ScanMode {
  Full,
  //// vector of module id
  Partial(Vec<ArcStr>),
}
