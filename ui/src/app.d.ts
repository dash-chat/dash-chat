declare namespace App {
	interface PageState {
		sidebarPanel?: 'new-message';
	}
}

interface Window {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	__setLocale: any;
}
