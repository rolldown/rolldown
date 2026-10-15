/* #__NO_SIDE_EFFECTS__ */
function aliased(value) {
  console.log(value);
  return [];
}

export { aliased as alias };
