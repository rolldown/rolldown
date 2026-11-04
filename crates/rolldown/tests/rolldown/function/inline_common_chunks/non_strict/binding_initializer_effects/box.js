globalThis.events.push('box body');
export class Box {
  constructor(name) {
    globalThis.events.push('new ' + name);
    this.name = name;
  }
}
