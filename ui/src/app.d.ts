declare namespace App {
	interface PageState {
		sidebarPanel?: 'new-message';
		stagedMedia?: true;
		lightbox?: true;
	}
}

interface ImportMetaEnv {
	/** Set by the e2e harness when it builds the binary under test, so
	 * development-only chrome can be compiled out of it. */
	readonly VITE_E2E?: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
