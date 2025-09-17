export var character = 0

export function next() {
  character++
  return character
}

export function noop() {}

export let immutable_let = "immutable_let";
export let mutable_let = "mutable_let";


mutable_let = "mutable_let1";
