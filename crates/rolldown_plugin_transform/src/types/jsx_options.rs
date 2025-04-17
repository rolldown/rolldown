use itertools::Either;

#[derive(Debug, Clone)]
pub struct JsxOptions {
  /// Decides which runtime to use.
  ///
  /// - 'automatic' - auto-import the correct JSX factories
  /// - 'classic' - no auto-import
  ///
  /// @default 'automatic'
  pub runtime: Option<String>,

  /// Emit development-specific information, such as `__source` and `__self`.
  ///
  /// @default false
  ///
  /// @see {@link https://babeljs.io/docs/babel-plugin-transform-react-jsx-development}
  pub development: Option<bool>,

  /// Toggles whether or not to throw an error if an XML namespaced tag name
  /// is used.
  ///
  /// Though the JSX spec allows this, it is disabled by default since React's
  /// JSX does not currently have support for it.
  ///
  /// @default true
  pub throw_if_namespace: Option<bool>,

  /// Enables `@babel/plugin-transform-react-pure-annotations`.
  ///
  /// It will mark top-level React method calls as pure for tree shaking.
  ///
  /// @see {@link https://babeljs.io/docs/en/babel-plugin-transform-react-pure-annotations}
  ///
  /// @default true
  pub pure: Option<bool>,

  /// Replaces the import source when importing functions.
  ///
  /// @default 'react'
  pub import_source: Option<String>,

  /// Replace the function used when compiling JSX expressions. It should be a
  /// qualified name (e.g. `React.createElement`) or an identifier (e.g.
  /// `createElement`).
  ///
  /// Only used for `classic` {@link runtime}.
  ///
  /// @default 'React.createElement'
  pub pragma: Option<String>,

  /// Replace the component used when compiling JSX fragments. It should be a
  /// valid JSX tag name.
  ///
  /// Only used for `classic` {@link runtime}.
  ///
  /// @default 'React.Fragment'
  pub pragma_frag: Option<String>,

  /// When spreading props, use `Object.assign` directly instead of an extend helper.
  ///
  /// Only used for `classic` {@link runtime}.
  ///
  /// @default false
  pub use_built_ins: Option<bool>,

  /// When spreading props, use inline object with spread elements directly
  /// instead of an extend helper or Object.assign.
  ///
  /// Only used for `classic` {@link runtime}.
  ///
  /// @default false
  pub use_spread: Option<bool>,

  /// Enable React Fast Refresh .
  ///
  /// Conforms to the implementation in {@link https://github.com/facebook/react/tree/v18.3.1/packages/react-refresh}
  ///
  /// @default false
  pub refresh: Option<Either<bool, ReactRefreshOptions>>,
}

#[derive(Debug, Clone)]
pub struct ReactRefreshOptions {
  /// Specify the identifier of the refresh registration variable.
  ///
  /// @default `$RefreshReg$`.
  pub refresh_reg: Option<String>,

  /// Specify the identifier of the refresh signature variable.
  ///
  /// @default `$RefreshSig$`.
  pub refresh_sig: Option<String>,

  pub emit_full_signatures: Option<bool>,
}
