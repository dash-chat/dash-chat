declare namespace App {
	interface PageState {
		sidebarPanel?: 'new-message';
		contentPanel?: 'add-contact';
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

interface Window {
	/** What `@tauri-apps/api` talks to: Tauri's own in the app, a stand-in in
	 * the browser preview, which says so with `preview`. */
	__TAURI_INTERNALS__?: {
		preview?: boolean;
		convertFileSrc?: () => string;
		invoke?: (
			command: string,
			args: Record<string, unknown>,
		) => Promise<unknown>;
		runCallback?: (id: number, data: unknown) => void;
	};
	/** The fake backend of the browser preview, when it is not in the page. */
	__fakeBackend?: (
		command: string,
		args: Record<string, unknown>,
	) => Promise<unknown>;
}
