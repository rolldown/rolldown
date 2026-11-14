function consumerHelper(input) {
  if (input > 10) {
    throw new Error('consumer-inner-anchor');
  }
  return input * 2;
}
globalThis.__events.push('consumer-anchor', consumerHelper(globalThis.__input));
