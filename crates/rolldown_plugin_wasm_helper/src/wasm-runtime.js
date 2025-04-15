export default async function wasmHelper(opts, url) {
  let result;
  if (url.startsWith('data:')) {
    const urlContent = url.replace(/^data:.*?base64,/, '');
    let bytes;
    if (typeof Buffer === 'function' && typeof Buffer.from === 'function') {
      bytes = Buffer.from(urlContent, 'base64');
    } else if (typeof atob === 'function') {
      const binaryString = atob(urlContent);
      bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }
    } else {
      throw new Error(
        'Failed to decode base64-encoded data URL, Buffer and atob are not supported',
      );
    }
    result = await WebAssembly.instantiate(bytes, opts);
  } else {
    // https://github.com/mdn/webassembly-examples/issues/5
    // WebAssembly.instantiateStreaming requires the server to provide the
    // correct MIME type for .wasm files, which unfortunately doesn't work for
    // a lot of static file servers, so we just work around it by getting the
    // raw buffer.
    const response = await fetch(url);
    const contentType = response.headers.get('Content-Type') || '';
    if (
      'instantiateStreaming' in WebAssembly &&
      contentType.startsWith('application/wasm')
    ) {
      result = await WebAssembly.instantiateStreaming(response, opts);
    } else {
      const buffer = await response.arrayBuffer();
      result = await WebAssembly.instantiate(buffer, opts);
    }
  }
  return result.instance;
}
