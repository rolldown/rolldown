function capture() {
  if (globalThis.marker === 'before') {
    throw new Error('before');
  }
}
exports.SNAPSHOT = /* @__PURE__ */ capture();
