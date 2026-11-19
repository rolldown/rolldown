Object.defineProperty(exports, 'value', {
  get() {
    globalThis.calls.push('getter');
    return 0;
  },
});
