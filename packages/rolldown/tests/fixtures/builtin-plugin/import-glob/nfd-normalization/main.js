// The glob pattern uses NFC form (normal source code encoding).
// On macOS, the directory is stored in NFD form by the filesystem.
const modules = import.meta.glob('./\u30DD/*.js');

export { modules };
