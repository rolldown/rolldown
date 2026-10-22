import { late, ready, state } from './tla.js';
import { evalFlag, readEval, run } from './eval.js';

globalThis.log = [];
const log = (msg) => globalThis.log.push(msg);

// A module with top-level await finishes before the modules that import it run.
if (ready) log('LIVE_tla_binding');
if (state.ready) log('LIVE_tla_property');
if (late() === 'set') log('LIVE_tla_function');

// A direct `eval` in included code can read and write each binding of its module.
run();
log(readEval());
if (evalFlag) log('LIVE_eval_binding');

export const done = true;
