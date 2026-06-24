function reassigned() {}

reassigned = function () {
  globalThis.reassignedSideEffect = true;
};

export { reassigned };
