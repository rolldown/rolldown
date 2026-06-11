// Single entry for the lazy-compilation playground. Each scenario is a co-tenant
// module that wires up its own button + DOM nodes; importing them here does NOT
// warm their lazy chunks (compilation is lazy — a chunk compiles only when its
// dynamic import fires), so each spec still sees a virgin first-fetch for the one
// scenario it exercises.
import './basic/setup.js';
import './aliased-import/setup.js';
import './shared-module/setup.js';
import './nested-dynamic-import/setup.js';
