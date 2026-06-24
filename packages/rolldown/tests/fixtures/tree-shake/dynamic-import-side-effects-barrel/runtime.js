export function createApp(app) {
  app();
}

export function createVNode(tag, props, children) {
  return { tag, props, children };
}

const extend = Object.assign;
const isFunction = (value) => typeof value === 'function';

// @__NO_SIDE_EFFECTS__
function defineComponent(options, extraOptions) {
  return isFunction(options)
    ? /* @__PURE__ */ (() => extend({ name: options.name }, extraOptions, { setup: options }))()
    : options;
}

export function isVNode(value) {
  return !!value;
}

export function unref(value) {
  return value;
}

export { defineComponent };
