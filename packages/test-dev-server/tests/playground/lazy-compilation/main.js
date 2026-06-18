// Single entry for the lazy-compilation playground. Each scenario sets up its
// own button and DOM nodes. Importing them here does not compile their lazy
// chunks — a lazy chunk compiles only when its dynamic import runs — so each
// spec still gets a fresh first fetch for its own scenario.
import './basic/setup.js';
import './aliased-import/setup.js';
import './shared-module/setup.js';
import './nested-dynamic-import/setup.js';
