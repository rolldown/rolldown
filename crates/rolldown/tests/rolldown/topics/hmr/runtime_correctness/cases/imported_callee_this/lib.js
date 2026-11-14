export function getThis() {
  return this;
}

export default function getThisDefault() {
  return this;
}

export { getThis as 'get-this' };

import 'trigger-dep';
