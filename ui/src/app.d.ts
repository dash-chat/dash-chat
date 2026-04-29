declare namespace App {
	interface PageState {
		sidebarPanel?: 'new-message';
	}
}

interface Window {
	__setLocale: typeof import('$lib/paraglide/runtime').setLocale;
}
