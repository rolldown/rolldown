export class B {
  static x = ((globalThis.__bStaticRan = true), [1, 2, 3].map(String));
}
