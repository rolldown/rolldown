export function dec(value, ctx) {
  globalThis.__decCalls = (globalThis.__decCalls ?? 0) + 1;
}
@dec
export class Decorated {
  @dec m() {}
}
export class OnlyAccessor {
  accessor a = 1;
  static accessor b = 2;
}
export class OnlyMemberDecorated {
  @dec field = 1;
}
export class Used {
  accessor v = 3;
}
