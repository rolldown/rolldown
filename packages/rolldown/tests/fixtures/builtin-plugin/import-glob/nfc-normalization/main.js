// The glob pattern uses NFD form (decomposed: U+30DB + U+309A).
// On disk, the directory may be stored in NFC form (precomposed: U+30DD).
const modules = import.meta.glob('./\u30DB\u309A/*.js');

export { modules };
