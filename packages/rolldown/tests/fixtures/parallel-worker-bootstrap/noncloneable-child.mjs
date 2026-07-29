import nodePath from 'node:path';
import { rolldown } from 'rolldown';
import { defineParallelPlugin } from 'rolldown/experimental';

const disruptReporting = process.argv.includes('--disrupt-reporting');
const plugin = defineParallelPlugin(nodePath.join(import.meta.dirname, 'noncloneable-plugin.mjs'));

let bundle;
try {
  bundle = await rolldown({
    cwd: import.meta.dirname,
    input: 'input.js',
    plugins: [plugin({ disruptReporting })],
  });
  await bundle.generate();
  throw new Error('parallel worker bootstrap unexpectedly succeeded');
} catch (error) {
  if (!containsMessage(error, 'parallel bootstrap')) {
    throw error;
  }
  console.log(
    disruptReporting
      ? 'parallel worker reporting capability isolated'
      : 'parallel worker non-cloneable failure reported',
  );
} finally {
  await bundle?.close().catch(() => {});
}

function containsMessage(error, expected) {
  if (String(error?.message ?? error).includes(expected)) return true;
  return (
    error instanceof AggregateError &&
    error.errors.some((nestedError) => containsMessage(nestedError, expected))
  );
}
