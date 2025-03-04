import assert from 'node:assert';
function parseImportMap(importMapJSONString, baseURL) {
  const parsed = JSON.parse(importMapJSONString);
  if (typeof parsed !== 'object' || !parsed) {
    throw new TypeError(
      'parseImportMap: Top-level value needs to be a JSON object'
    );
  }
  let sortedAndNormalizedImports = /* @__PURE__ */ new Map();
  const imports = parsed['imports'];
  if (imports) {
    if (typeof imports !== 'object') {
      throw new TypeError(
        'parseImportMap: "imports" top-level key needs to be a JSON object'
      );
    }
    sortedAndNormalizedImports = sortAndNormalizeSpecifierMap(imports, baseURL);
  }
  let sortedAndNormalizedScopes = /* @__PURE__ */ new Map();
  const scopes = parsed['scopes'];
  if (scopes) {
    if (typeof scopes !== 'object') {
      throw new TypeError(
        'parseImportMap: "scopes" top-level key needs to be a JSON object'
      );
    }
    sortedAndNormalizedScopes = sortAndNormalizeScopes(scopes, baseURL);
  }
  for (const key of Object.keys(parsed)) {
    if (key !== 'imports' && key !== 'scopes') {
      console.warn(`parseImportMap: Invalid top-level key "${key}"`);
    }
  }
  return {
    imports: sortedAndNormalizedImports,
    scopes: sortedAndNormalizedScopes,
  };
}
function sortAndNormalizeSpecifierMap(originalMap, baseURL) {
  const normalized = /* @__PURE__ */ new Map();
  for (const [specifierKey, value] of Object.entries(originalMap)) {
    const normalizedSpecifierKey = normalizeSpecifierKey(specifierKey, baseURL);
    if (normalizedSpecifierKey === null) continue;
    if (typeof value !== 'string') {
      console.warn('parseImportMap: addresses need to be strings');
      normalized.set(normalizedSpecifierKey, null);
      continue;
    }
    const addressURL = parseURLLikeImportSpecifier(value, baseURL);
    if (addressURL === null) {
      console.warn(`parseImportMap: address "${value}" is invalid`);
      normalized.set(normalizedSpecifierKey, null);
      continue;
    }
    if (specifierKey.endsWith('/') && !addressURL.toString().endsWith('/')) {
      console.warn(
        `parseImportMap: invalid address for specifier "${specifierKey}"; since specifier ends in a slash, the address needs to as well`
      );
      normalized.set(normalizedSpecifierKey, null);
      continue;
    }
    normalized.set(normalizedSpecifierKey, addressURL);
  }
  const normalizedAndSortedEntries = Array.from(normalized);
  normalizedAndSortedEntries.sort(revertSortKeyComparator);
  return new Map(normalizedAndSortedEntries);
}
function sortAndNormalizeScopes(originalMap, baseURL) {
  const normalized = /* @__PURE__ */ new Map();
  for (const [scopePrefix, potentialSpecifierMap] of Object.entries(
    originalMap
  )) {
    if (typeof potentialSpecifierMap !== 'object' || !potentialSpecifierMap) {
      throw new TypeError(
        `parseImportMap: value of the scope with prefix "${scopePrefix}" needs to be a JSON object`
      );
    }
    try {
      const scopePrefixURL = new URL(scopePrefix, baseURL || void 0);
      const normalizedScopePrefix = scopePrefixURL.toString();
      normalized.set(
        normalizedScopePrefix,
        sortAndNormalizeSpecifierMap(potentialSpecifierMap, baseURL)
      );
    } catch {
      console.warn(
        `parseImportMap: scope prefix "${scopePrefix}" was not parseable`
      );
    }
  }
  const normalizedAndSortedEntries = Array.from(normalized);
  normalizedAndSortedEntries.sort(revertSortKeyComparator);
  return new Map(normalizedAndSortedEntries);
}
function normalizeSpecifierKey(specifierKey, baseURL) {
  if (specifierKey === '') {
    console.warn('parseImportMap: specifier keys cannot be the empty string');
    return null;
  }
  const url = parseURLLikeImportSpecifier(specifierKey, baseURL);
  if (url) return url.toString();
  return specifierKey;
}
function revertSortKeyComparator(a, b) {
  const aKey = a[0];
  const bKey = b[0];
  if (aKey < bKey) return 1;
  else if (aKey > bKey) return -1;
  else return 0;
}
class UnresolvableSpecifierError extends TypeError {
  constructor(specifier) {
    super(
      `Bare specifier "${specifier}" was not remapped to anything by importMap`
    );
    this.specifier = specifier;
  }
}
function resolveModuleSpecifier(importMap, specifier, baseURL) {
  const baseURLString = baseURL && baseURL.toString();
  const asURL = parseURLLikeImportSpecifier(specifier, baseURL);
  const normalizedSpecifier = asURL?.toString() || specifier;
  if (baseURLString) {
    for (const [scopePrefix, scopeImports] of importMap.scopes) {
      if (
        scopePrefix === baseURLString ||
        (scopePrefix.endsWith('/') && baseURLString.startsWith(scopePrefix))
      ) {
        const scopeImportsMatch = resolveImportsMatch(
          normalizedSpecifier,
          asURL,
          scopeImports
        );
        if (scopeImportsMatch) return scopeImportsMatch;
      }
    }
  }
  const topLevelImportsMatch = resolveImportsMatch(
    normalizedSpecifier,
    asURL,
    importMap.imports
  );
  if (topLevelImportsMatch) return topLevelImportsMatch;
  if (asURL) return asURL;
  throw new UnresolvableSpecifierError(specifier);
}
function resolveImportsMatch(normalizedSpecifier, asURL, specifierMap) {
  for (const [specifierKey, resolutionResult] of specifierMap) {
    if (specifierKey === normalizedSpecifier) {
      return resolutionResult;
    } else if (
      specifierKey.endsWith('/') &&
      normalizedSpecifier.startsWith(specifierKey) &&
      (asURL === null || isSpecialURL(asURL))
    ) {
      if (resolutionResult === null) {
        throw new TypeError(
          `Resolution of "${specifierKey}" was blocked by a null entry`
        );
      }
      const afterPrefix = normalizedSpecifier.split(specifierKey, 2)[1];
      assert(
        resolutionResult.toString().endsWith('/'),
        'Expected resolutionResult to end in a slash, as enforced during parsing'
      );
      let url;
      try {
        url = new URL(afterPrefix, resolutionResult);
      } catch {
        throw new TypeError(
          `Resolution of "${normalizedSpecifier}" was blocked since the "${afterPrefix}" portion could not be URL-parsed relative to the "${resolutionResult}" mapped to by the "${specifierKey}" prefix`
        );
      }
      if (!url.toString().startsWith(resolutionResult.toString())) {
        throw new TypeError(
          `Resolution of "${normalizedSpecifier}" was blocked due to it backtracking above its prefix "${specifierKey}"`
        );
      }
      return url;
    }
  }
  return null;
}
function parseURLLikeImportSpecifier(specifier, baseURL) {
  if (
    specifier.startsWith('/') ||
    specifier.startsWith('./') ||
    specifier.startsWith('../')
  ) {
    if (!baseURL) {
      throw new Error(
        `Specifier "${specifier}" starts with /, ./, or ../, but baseURL is null`
      );
    }
    try {
      const url = new URL(specifier, baseURL);
      return url;
    } catch {
      return null;
    }
  }
  try {
    const url = new URL(specifier);
    return url;
  } catch {
    return null;
  }
}
function isSpecialURL(url) {
  return ['ftp:', 'file:', 'http:', 'https:', 'ws:', 'wss:'].includes(
    url.protocol
  );
}
export { UnresolvableSpecifierError, parseImportMap, resolveModuleSpecifier };
