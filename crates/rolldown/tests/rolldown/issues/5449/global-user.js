// Uses the global `process`; the external binding hoisted for process-importer.js must not shadow it.
process.on('beforeExit', () => {});
globalThis.__globalProcessOk = true;
