use rolldown_common::Platform;

pub fn binary_to_esm(base64: &str, platform: Platform, runtime_module_id: &str) -> String {
  let to_binary = match platform {
    Platform::Node => "__toBinaryNode",
    _ => "__toBinary",
  };
  [
    "import {",
    to_binary,
    "} from '",
    runtime_module_id,
    "'; export default ",
    to_binary,
    "('",
    base64,
    "')",
  ]
  .concat()
}
