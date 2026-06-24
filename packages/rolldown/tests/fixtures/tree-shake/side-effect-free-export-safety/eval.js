function evalReassigned() {}

eval('evalReassigned = () => { globalThis.evalHit = true }');

export { evalReassigned };
