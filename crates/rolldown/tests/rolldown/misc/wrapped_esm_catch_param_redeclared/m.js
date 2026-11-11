var e = 0;
try {
  throw 1;
} catch (e) {
  var e = 2;
  globalThis.caught = e;
}
e += 10;

try {
  throw 3;
} catch (f) {
  var f = 4;
}

try {
  throw 5;
} catch (g) {
  try {
    throw 6;
  } catch (g) {
    var g = 7;
  }
}

export { e, f, g };
