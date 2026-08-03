#!/usr/bin/env node
// `rolldown-metrics` — launcher for the metrics-lab perf harness that lives next
// to rolldown in the monorepo. When rolldown is installed via `link:` (a source
// checkout), this file's real path sits inside that checkout, so the lab is a
// sibling package and needs no install of its own. The published rolldown package
// doesn't bundle the lab; point users at @rolldown/metrics-lab instead.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

// Resolve through any node_modules junction/symlink to the real checkout.
const self = fs.realpathSync(fileURLToPath(import.meta.url));
const harness = path.resolve(path.dirname(self), '..', '..', 'metrics-lab', 'harness.mjs');

if (!fs.existsSync(harness)) {
  console.error(
    'rolldown-metrics: the metrics-lab harness is not part of the published rolldown package.',
  );
  console.error(
    'It is available when rolldown is linked from a source checkout, or as its own package:',
  );
  console.error('  npm i -D @rolldown/metrics-lab   then   npx metrics-lab scan --app .');
  process.exit(1);
}

// Keep measurement state in the CALLER'S project, not in the checkout, and make
// the harness's next-step hints spell this invocation.
process.env.METRICS_LAB_STATE ??= path.join(process.cwd(), '.metrics-lab');
process.env.METRICS_LAB_CLI ??= 'npx rolldown-metrics';

await import(pathToFileURL(harness).href);
