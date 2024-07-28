try {
	oldLocale = globalLocale._abbr;
	var aliasedRequire = require;
	aliasedRequire('./locale/' + name);
	getSetGlobalLocale(oldLocale);
} catch (e) {}