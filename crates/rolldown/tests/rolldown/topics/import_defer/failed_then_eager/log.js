export const log = [];
export function read(access) {
  try {
    access();
    return 'ok';
  } catch (error) {
    return error instanceof TypeError ? 'TypeError' : `${error.constructor.name}: ${error.message}`;
  }
}
