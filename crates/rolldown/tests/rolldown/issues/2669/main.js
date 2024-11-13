export function nested1() {
  function x() {}
  function x$1() {
    x()
  }
  function x$1$1() {
    x()
  }
  return [x, x$1, x$1$1];
}

export function nested2() {
  function x$1$1() {
    x()
  }
  function x$1() {
    x()
  }
  function x() {}
  return [x, x$1, x$1$1];
}

export const x = "x";
