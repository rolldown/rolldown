// Step 0 breaks dep.js (queued in `pending_rescans`). Step 1 touches side.txt,
// a watched file that maps to no module — that step must retry dep.js so the
// repeated failure keeps `last_task_errored` set. Step 2 restores dep.js to
// its original bytes; the latch keeps the unchanged-output suppression off,
// so the recovery patch still ships.
import { value } from './dep'
console.log(value)
