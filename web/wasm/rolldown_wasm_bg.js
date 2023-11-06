let wasm
export function __wbg_set_wasm(val) {
  wasm = val
}

const heap = new Array(128).fill(undefined)

heap.push(undefined, null, true, false)

function getObject(idx) {
  return heap[idx]
}

let heap_next = heap.length

function dropObject(idx) {
  if (idx < 132) return
  heap[idx] = heap_next
  heap_next = idx
}

function takeObject(idx) {
  const ret = getObject(idx)
  dropObject(idx)
  return ret
}

const lTextDecoder =
  typeof TextDecoder === 'undefined'
    ? (0, module.require)('util').TextDecoder
    : TextDecoder

let cachedTextDecoder = new lTextDecoder('utf-8', {
  ignoreBOM: true,
  fatal: true,
})

cachedTextDecoder.decode()

let cachedUint8Memory0 = null

function getUint8Memory0() {
  if (cachedUint8Memory0 === null || cachedUint8Memory0.byteLength === 0) {
    cachedUint8Memory0 = new Uint8Array(wasm.memory.buffer)
  }
  return cachedUint8Memory0
}

function getStringFromWasm0(ptr, len) {
  ptr = ptr >>> 0
  return cachedTextDecoder.decode(getUint8Memory0().subarray(ptr, ptr + len))
}

let WASM_VECTOR_LEN = 0

const lTextEncoder =
  typeof TextEncoder === 'undefined'
    ? (0, module.require)('util').TextEncoder
    : TextEncoder

let cachedTextEncoder = new lTextEncoder('utf-8')

const encodeString =
  typeof cachedTextEncoder.encodeInto === 'function'
    ? function (arg, view) {
        return cachedTextEncoder.encodeInto(arg, view)
      }
    : function (arg, view) {
        const buf = cachedTextEncoder.encode(arg)
        view.set(buf)
        return {
          read: arg.length,
          written: buf.length,
        }
      }

function passStringToWasm0(arg, malloc, realloc) {
  if (realloc === undefined) {
    const buf = cachedTextEncoder.encode(arg)
    const ptr = malloc(buf.length, 1) >>> 0
    getUint8Memory0()
      .subarray(ptr, ptr + buf.length)
      .set(buf)
    WASM_VECTOR_LEN = buf.length
    return ptr
  }

  let len = arg.length
  let ptr = malloc(len, 1) >>> 0

  const mem = getUint8Memory0()

  let offset = 0

  for (; offset < len; offset++) {
    const code = arg.charCodeAt(offset)
    if (code > 0x7f) break
    mem[ptr + offset] = code
  }

  if (offset !== len) {
    if (offset !== 0) {
      arg = arg.slice(offset)
    }
    ptr = realloc(ptr, len, (len = offset + arg.length * 3), 1) >>> 0
    const view = getUint8Memory0().subarray(ptr + offset, ptr + len)
    const ret = encodeString(arg, view)

    offset += ret.written
  }

  WASM_VECTOR_LEN = offset
  return ptr
}

function addHeapObject(obj) {
  if (heap_next === heap.length) heap.push(heap.length + 1)
  const idx = heap_next
  heap_next = heap[idx]

  heap[idx] = obj
  return idx
}

let cachedInt32Memory0 = null

function getInt32Memory0() {
  if (cachedInt32Memory0 === null || cachedInt32Memory0.byteLength === 0) {
    cachedInt32Memory0 = new Int32Array(wasm.memory.buffer)
  }
  return cachedInt32Memory0
}

let cachedUint32Memory0 = null

function getUint32Memory0() {
  if (cachedUint32Memory0 === null || cachedUint32Memory0.byteLength === 0) {
    cachedUint32Memory0 = new Uint32Array(wasm.memory.buffer)
  }
  return cachedUint32Memory0
}

function passArrayJsValueToWasm0(array, malloc) {
  const ptr = malloc(array.length * 4, 4) >>> 0
  const mem = getUint32Memory0()
  for (let i = 0; i < array.length; i++) {
    mem[ptr / 4 + i] = addHeapObject(array[i])
  }
  WASM_VECTOR_LEN = array.length
  return ptr
}

function getArrayJsValueFromWasm0(ptr, len) {
  ptr = ptr >>> 0
  const mem = getUint32Memory0()
  const slice = mem.subarray(ptr / 4, ptr / 4 + len)
  const result = []
  for (let i = 0; i < slice.length; i++) {
    result.push(takeObject(slice[i]))
  }
  return result
}
/**
 * @param {(FileItem)[]} file_list
 * @returns {(AssetItem)[]}
 */
export function bundle(file_list) {
  try {
    const retptr = wasm.__wbindgen_add_to_stack_pointer(-16)
    const ptr0 = passArrayJsValueToWasm0(file_list, wasm.__wbindgen_malloc)
    const len0 = WASM_VECTOR_LEN
    wasm.bundle(retptr, ptr0, len0)
    var r0 = getInt32Memory0()[retptr / 4 + 0]
    var r1 = getInt32Memory0()[retptr / 4 + 1]
    var v2 = getArrayJsValueFromWasm0(r0, r1).slice()
    wasm.__wbindgen_free(r0, r1 * 4, 4)
    return v2
  } finally {
    wasm.__wbindgen_add_to_stack_pointer(16)
  }
}

/**
 */
export class AssetItem {
  static __wrap(ptr) {
    ptr = ptr >>> 0
    const obj = Object.create(AssetItem.prototype)
    obj.__wbg_ptr = ptr

    return obj
  }

  __destroy_into_raw() {
    const ptr = this.__wbg_ptr
    this.__wbg_ptr = 0

    return ptr
  }

  free() {
    const ptr = this.__destroy_into_raw()
    wasm.__wbg_assetitem_free(ptr)
  }
  /**
   * @returns {string}
   */
  get name() {
    let deferred1_0
    let deferred1_1
    try {
      const retptr = wasm.__wbindgen_add_to_stack_pointer(-16)
      wasm.assetitem_name(retptr, this.__wbg_ptr)
      var r0 = getInt32Memory0()[retptr / 4 + 0]
      var r1 = getInt32Memory0()[retptr / 4 + 1]
      deferred1_0 = r0
      deferred1_1 = r1
      return getStringFromWasm0(r0, r1)
    } finally {
      wasm.__wbindgen_add_to_stack_pointer(16)
      wasm.__wbindgen_free(deferred1_0, deferred1_1, 1)
    }
  }
  /**
   * @returns {string}
   */
  get content() {
    let deferred1_0
    let deferred1_1
    try {
      const retptr = wasm.__wbindgen_add_to_stack_pointer(-16)
      wasm.assetitem_content(retptr, this.__wbg_ptr)
      var r0 = getInt32Memory0()[retptr / 4 + 0]
      var r1 = getInt32Memory0()[retptr / 4 + 1]
      deferred1_0 = r0
      deferred1_1 = r1
      return getStringFromWasm0(r0, r1)
    } finally {
      wasm.__wbindgen_add_to_stack_pointer(16)
      wasm.__wbindgen_free(deferred1_0, deferred1_1, 1)
    }
  }
}
/**
 */
export class FileItem {
  static __unwrap(jsValue) {
    if (!(jsValue instanceof FileItem)) {
      return 0
    }
    return jsValue.__destroy_into_raw()
  }

  __destroy_into_raw() {
    const ptr = this.__wbg_ptr
    this.__wbg_ptr = 0

    return ptr
  }

  free() {
    const ptr = this.__destroy_into_raw()
    wasm.__wbg_fileitem_free(ptr)
  }
  /**
   * @param {string} path
   * @param {string} content
   */
  constructor(path, content) {
    const ptr0 = passStringToWasm0(
      path,
      wasm.__wbindgen_malloc,
      wasm.__wbindgen_realloc,
    )
    const len0 = WASM_VECTOR_LEN
    const ptr1 = passStringToWasm0(
      content,
      wasm.__wbindgen_malloc,
      wasm.__wbindgen_realloc,
    )
    const len1 = WASM_VECTOR_LEN
    const ret = wasm.fileitem_new(ptr0, len0, ptr1, len1)
    this.__wbg_ptr = ret >>> 0
    return this
  }
}

export function __wbg_fileitem_unwrap(arg0) {
  const ret = FileItem.__unwrap(takeObject(arg0))
  return ret
}

export function __wbg_assetitem_new(arg0) {
  const ret = AssetItem.__wrap(arg0)
  return addHeapObject(ret)
}

export function __wbindgen_object_drop_ref(arg0) {
  takeObject(arg0)
}

export function __wbg_new_abda76e883ba8a5f() {
  const ret = new Error()
  return addHeapObject(ret)
}

export function __wbg_stack_658279fe44541cf6(arg0, arg1) {
  const ret = getObject(arg1).stack
  const ptr1 = passStringToWasm0(
    ret,
    wasm.__wbindgen_malloc,
    wasm.__wbindgen_realloc,
  )
  const len1 = WASM_VECTOR_LEN
  getInt32Memory0()[arg0 / 4 + 1] = len1
  getInt32Memory0()[arg0 / 4 + 0] = ptr1
}

export function __wbg_error_f851667af71bcfc6(arg0, arg1) {
  let deferred0_0
  let deferred0_1
  try {
    deferred0_0 = arg0
    deferred0_1 = arg1
    console.error(getStringFromWasm0(arg0, arg1))
  } finally {
    wasm.__wbindgen_free(deferred0_0, deferred0_1, 1)
  }
}

export function __wbindgen_throw(arg0, arg1) {
  throw new Error(getStringFromWasm0(arg0, arg1))
}
