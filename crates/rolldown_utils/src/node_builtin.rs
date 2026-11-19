const NODE_BUILTIN_NAMESPACE: &str = "node:";
const BUN_BUILTIN_NAMESPACE: &str = "bun:";

/// Whether `id` is a builtin module of Node.js or Bun: a `node:` or `bun:` id, or a name in
/// `nodejs_built_in_modules::BUILTINS`.
pub fn is_node_like_builtin(id: &str) -> bool {
  id.starts_with(BUN_BUILTIN_NAMESPACE)
    || id.starts_with(NODE_BUILTIN_NAMESPACE)
    || nodejs_built_in_modules::BUILTINS.contains(&id)
}
