import assert from 'node:assert';
import { value } from './hmr.ts';

// Step 0 edits the tsconfig, so the engine reloads every client, and this client does not get
// that push. Step 1 edits `hmr.ts`. The client pulls with a generation from before the full
// reload, so the server answers with a reload, and the client runs the new output.
process.on('beforeExit', (code) => {
  if (code !== 0) return;
  assert.strictEqual(value, 'v2');
});
