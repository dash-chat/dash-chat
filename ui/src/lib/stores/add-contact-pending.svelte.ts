let pending = $state(false);

export const addContactPending = {
	get value() {
		return pending;
	},
	set value(v: boolean) {
		pending = v;
	},
};
