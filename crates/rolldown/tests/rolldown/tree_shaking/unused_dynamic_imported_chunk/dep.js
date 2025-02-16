console.log('dep');
export async function loadTS() {
  try {
    return import('./dynamic.js')
  } catch (e) {
    throw e;
  }
}
