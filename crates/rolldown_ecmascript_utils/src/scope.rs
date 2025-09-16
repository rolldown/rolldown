use oxc::semantic::ScopeFlags;

/// if current visit path is top level
/// including such scenario:
/// ```js
/// class T {
///   [foo]() {}
/// }
/// class A {
///   static {
///     foo;
///   }
/// }
///
/// foo;
/// {
///   foo;
/// }
/// ```
pub fn is_top_level(scope_stack: &[ScopeFlags]) -> bool {
  scope_stack.iter().rev().all(|flag| {
    flag.intersects(ScopeFlags::Top | ScopeFlags::StrictMode | ScopeFlags::ClassStaticBlock)
      || flag.is_empty()
  })
}
