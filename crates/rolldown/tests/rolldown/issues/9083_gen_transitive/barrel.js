// export { tlaValue } is NOT imported by any consumer → its stmt will
// be excluded (is_stmt_included=false) while barrel itself is still
// WrapKind::Esm.  This exercises the generate_transitive_esm_init()
// path that must emit `await init_deep()` rather than `init_deep()`.
export { tlaValue } from './deep.js';
// export { syncValue } IS used → exercises transform_or_remove path
export { syncValue } from './sync.js';
