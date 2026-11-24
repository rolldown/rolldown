export function identity(value) {
  return value;
}

export function readProperty({ state }) {
  return state;
}

export function throughThis(callback) {
  return this === undefined && callback();
}

export function throughArguments(callback) {
  return arguments[0]();
}
