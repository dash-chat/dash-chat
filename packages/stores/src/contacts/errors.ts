// Common error variants shared across multiple error types
export type Error =
	| { kind: 'InitializeTopic'; message: string }
	| { kind: 'AuthorOperation'; message: string };

export type AddContactError =
	| { kind: 'ProfileNotCreated'; message: null }
	| { kind: 'CreateQrCode'; message: string }
	| { kind: 'CreateDirectChat'; message: string }
	| Error;
