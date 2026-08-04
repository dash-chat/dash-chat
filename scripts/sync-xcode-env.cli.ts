import { MANAGED, type ManagedKey, syncXcodeEnv } from './sync-xcode-env.ts';

syncXcodeEnv(
	Object.fromEntries(MANAGED.map(k => [k, process.env[k]])) as Partial<
		Record<ManagedKey, string>
	>,
);
