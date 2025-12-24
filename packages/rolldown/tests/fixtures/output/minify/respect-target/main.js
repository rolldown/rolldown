export function hello(opts) {
  console.log(opts?.name);

  opts == null ? void 0 : opts.name;
}
