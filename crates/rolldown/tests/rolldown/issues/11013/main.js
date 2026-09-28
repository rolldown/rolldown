export async function f() {
  const id = "no-such-pkg-a";
  const longerName = "no-such-pkg-b";
  const a = "no-such-pkg-d";
  const existing = "./dep.js";
  const required = "no-such-pkg-e";
  return [
    await import(id),
    await import(longerName),
    await import("no-such-pkg-c"),
    await import(a),
    await import(existing),
    await import(`no-such-pkg-f`),
    await import("no-such-" + "pkg-g"),
    await import("no-such-\u0070kg-h"),
    require(required),
  ];
}
