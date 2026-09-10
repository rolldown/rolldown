// Workerd bundles alias `src/timer-host.ts` to this empty module. The real
// module registers process-wide CurrentThread task/timer hosts against the
// ambient binding at module evaluation, which workerd builds must never do:
// the managed deferred loader (`rolldown-binding.wasip1-deferred.js`)
// registers per-instance task and timer hosts itself during createInstance()
// and hides the private host exports from the instance facade.
export {};
