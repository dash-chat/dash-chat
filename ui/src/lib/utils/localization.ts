// Import all webawesome translations for all supported languages
import "@awesome.me/webawesome/dist/translations/es.js";
import "@awesome.me/webawesome/dist/translations/en.js";
import "@awesome.me/webawesome/dist/translations/fa.js";
import "@awesome.me/webawesome/dist/translations/de.js";

export const localesWithName: Array<{ locale: string; name: string }> = [
	{ locale: 'en', name: 'English' },
	{ locale: 'es', name: 'Español' },
	{ locale: 'de-de', name: 'Deutsch' },
	{ locale: 'fa-ir', name: 'فارسی' },
];

export const rtlLocales = new Set(['fa-ir']);
