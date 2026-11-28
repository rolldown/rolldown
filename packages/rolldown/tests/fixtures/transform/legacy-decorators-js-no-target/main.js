function dec(target, key) {
  target.decorated = key;
}

export class C {
  @dec
  m() {}
}
