export const names = [];
try {
  throw 0;
} catch (e) {
  var e = 1;
  names.push(e);
}
{
  function e() {}
  names.push(e.name);
}
{
  class e {}
  names.push(e.name);
}
{
  let e = function () {};
  names.push(e.name);
}
{
  const e = class {};
  names.push(e.name);
}
export { e as outer };
