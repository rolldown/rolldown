#### In-depth

Multiple JavaScript engines have had and continue to have performance issues with [Temporal dead zone (TDZ)](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/let#temporal_dead_zone_tdz) checks. These checks validate that a let, const, or class symbol isn't used before it's initialized.

Related issues:

- V8: https://issues.chromium.org/issues/42203665
- JavaScriptCore: https://bugs.webkit.org/show_bug.cgi?id=199866 (fixed)
