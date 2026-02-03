// Common error variants shared across multiple error types
export type Error =
	| { kind: 'InitializeTopic'; message: string }
	| { kind: 'AuthorOperation'; message: string };

export type ContactCodeError =
	| { kind: 'GetContactCode'; message: string }
	| { kind: 'SetContactCode'; message: string }
	| { kind: 'ClearContactCode'; message: string }
	| Error;

export type AddContactError =
	| { kind: 'ProfileNotCreated'; message: null }
	| { kind: 'CreateContactCode'; message: string }
	| { kind: 'CreateDirectChat'; message: string }
	| Error;
