let replaced = /* #__NO_SIDE_EFFECTS__ */ () => {};
eval("replaced = () => globalThis.calls.push('eval')");

export { replaced };
