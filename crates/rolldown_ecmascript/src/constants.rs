use oxc::span::Atom;

// Special global variable `exports` in CommonJS modules
pub const CJS_EXPORTS_REF: &str = "exports";

// Special global variable `module` in CommonJS modules
pub const CJS_MODULE_REF: &str = "module";

// Special global variable `require` in CommonJS modules
pub const CJS_REQUIRE_REF: &str = "require";

// Rolldown will rewrite `exports` in CommonJS modules to this.
pub const CJS_ROLLDOWN_EXPORTS_REF: &str = "__rolldown_exports__";

// Rolldown will rewrite `module` in CommonJS modules to this.
pub const CJS_ROLLDOWN_MODULE_REF: &str = "__rolldown_module__";

pub const CJS_EXPORTS_REF_ATOM: Atom<'static> = Atom::new_const(CJS_EXPORTS_REF);
pub const CJS_MODULE_REF_ATOM: Atom<'static> = Atom::new_const(CJS_MODULE_REF);
pub const CJS_REQUIRE_REF_ATOM: Atom<'static> = Atom::new_const(CJS_REQUIRE_REF);
pub const CJS_ROLLDOWN_EXPORTS_REF_ATOM: Atom<'static> = Atom::new_const(CJS_ROLLDOWN_EXPORTS_REF);
pub const CJS_ROLLDOWN_MODULE_REF_ATOM: Atom<'static> = Atom::new_const(CJS_ROLLDOWN_MODULE_REF);
