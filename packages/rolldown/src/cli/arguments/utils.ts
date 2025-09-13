export function setNestedProperty<T extends object, K>(
  obj: T,
  path: string,
  value: K,
): void {
  const keys = path.split('.') as (keyof T)[];
  let current: any = obj;

  for (let i = 0; i < keys.length - 1; i++) {
    if (!current[keys[i]]) {
      current[keys[i]] = {};
    }
    current = current[keys[i]];
  }

  const finalKey = keys[keys.length - 1];
  Object.defineProperty(current, finalKey, {
    value: value,
    writable: true,
    enumerable: true,
    configurable: true,
  });
}

export function camelCaseToKebabCase(str: string): string {
  return str.replace(/[A-Z]/g, (match) => `-${match.toLowerCase()}`);
}

export function kebabCaseToCamelCase(str: string): string {
  return str.replace(/-./g, (match) => match[1].toUpperCase());
}
