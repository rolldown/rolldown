const extend = Object.assign
const isFunction = (val) => typeof val == "function";


/* #__NO_SIDE_EFFECTS__ */
export function defineComponent(options, extraOptions) {
  return isFunction(options)
    ? /* #__PURE__ */ (() => extend({}, extraOptions, { setup: options, name: options.name }))()
    : options
}

const _sfc_main = defineComponent({

});
export default _sfc_main
