#[macro_export]
macro_rules! append_injection {
  ($concat_source:ident, $( $injection_name:ident ),*) => {
    $(
      if let Some($injection_name) = $injection_name {
        $concat_source.add_source(Box::new(RawSource::new($injection_name)));
      }
    )*
  };
}
