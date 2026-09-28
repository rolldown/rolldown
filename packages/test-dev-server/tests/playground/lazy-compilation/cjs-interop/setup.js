// cjs-interop: `import()` of a CommonJS module must have `default` =
// `module.exports` (vitejs/vite#23558). `fetched.cjs` is compiled on demand;
// `resident.cjs` is already in the page through the static import below.
import Resident from './resident.cjs';

const log = (msg) => {
  document.getElementById('cjs-interop-log').textContent += msg + '\n';
};

const describe = (name, mod) => {
  const def = mod.default;
  log(`${name}.keys = ${Object.keys(mod).join(',')}`);
  log(`${name}.default = ${typeof def === 'function' ? def.version : 'UNDEFINED'}`);
  log(`${name}.version = ${mod.version}`);
};

document.getElementById('cjs-interop-btn').addEventListener('click', async () => {
  try {
    const fetched = await import('./fetched.cjs');
    describe('fetched', fetched);

    const resident = await import('./resident.cjs');
    describe('resident', resident);
    log(`resident.default === static import: ${resident.default === Resident}`);
  } catch (error) {
    log(`error: ${error}`);
  }
  document.getElementById('cjs-interop-status').textContent = 'done';
});
