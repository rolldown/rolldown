import { identity } from './lib.js';

const widget = {
  label: [
    {
      [identity({
        toString() {
          globalThis.calls.push('computed key');
          return 'key';
        },
      })]: 0,
    },
  ],
};
function wrap(value) {
  return value;
}
export const Widget = /* @__PURE__ */ wrap(widget);
