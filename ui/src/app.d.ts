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
	/** Whether the build has a Sentry DSN, so the report action is compiled out
	 * of builds that could not send it. */
	readonly VITE_SENTRY_ENABLED: boolean;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
