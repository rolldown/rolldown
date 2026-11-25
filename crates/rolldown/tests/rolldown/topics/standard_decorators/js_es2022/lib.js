export const log = [];
export function logged(value, ctx) {
  log.push(`${ctx.kind}:${String(ctx.name)}`);
  if (ctx.kind === 'method') {
    return function (...args) {
      log.push(`call:${String(ctx.name)}`);
      return value.call(this, ...args);
    };
  }
  if (ctx.kind === 'field') return (v) => v * 2;
  if (ctx.kind === 'accessor') {
    return {
      get() {
        return value.get.call(this) + 1;
      },
      set(v) {
        value.set.call(this, v);
      },
      init(v) {
        return v + 100;
      },
    };
  }
  if (ctx.kind === 'class') {
    ctx.metadata.tagged = true;
    ctx.addInitializer(function () {
      log.push(`init:${this.name}`);
    });
  }
}
