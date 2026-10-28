globalThis.originalDefineProperty = Object.defineProperty;
Object.defineProperty = () => {
  throw new Error('runtime captured the patched intrinsic');
};
