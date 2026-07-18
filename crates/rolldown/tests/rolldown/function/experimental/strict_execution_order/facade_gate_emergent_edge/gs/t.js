// Interop-wrapped (required by the CJS `sec`) and co-hosted with it. `tv` is consumed by `sec`;
// `unused` is imported only by the excluded forwarder `f`, so only the excluded-statement metadata
// reaches it — the projected hop the facade gate must see.
export const tv = /* @__PURE__ */ (() => 'TV')();
export const unused = 'UNUSED';
