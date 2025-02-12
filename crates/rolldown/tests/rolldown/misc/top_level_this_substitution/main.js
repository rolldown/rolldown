class BaseNode extends Callable {
  // should not be replaced
  referencesById = children.reduce((result, child) => Object.assign(result, child.referencesById), { [this.id]: this });
  // should replace
  [this.id] = this;
}

console.log(`BaseNode: `, BaseNode)

