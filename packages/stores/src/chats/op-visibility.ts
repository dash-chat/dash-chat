import {
	AuthGroupMember,
	SimplifiedOperation,
} from '../p2panda/simplified-types';
import { DeviceId } from '../p2panda/types';
import { Payload } from '../types';

/**
 * Client-side filtering of operations that should never have reached us.
 *
 * Leaving a group does not unsubscribe us from its topic and the backend
 * enforces no membership rule, so a group we left keeps delivering messages.
 * Hiding them here is a stopgap, not the design: a peer that keeps sending is
 * still reaching this device, and only the renderer looks away.
 *
 * TODO: delete this module once the backend stops accepting operations in a
 * topic we are no longer a member of — the same fix `dropOpsAuthoredWhileBlocked`
 * in `messages-store.ts` is waiting on, which needs rejected operations dropped
 * from the op store rather than merely skipping their notification.
 */

/** A point at which a boolean fact about a chat flipped. */
export type StateChange = { value: boolean; timestamp: number };

/** The value the latest change at or before `timestamp` left in place, or
 * `initial` when none applies. `history` must be sorted by ascending
 * timestamp. */
export function stateAt(
	history: StateChange[],
	timestamp: number,
	initial: boolean,
): boolean {
	let value = initial;
	for (const change of history) {
		if (change.timestamp > timestamp) break;
		value = change.value;
	}
	return value;
}

/** Drops chat ops authored while `myDeviceId` was outside this chat, so a
 * group carries on showing its history but nothing published after we left.
 *
 * Ops predating our first membership event are kept: a late joiner is given
 * the history from before she was added, and only a `Remove` ever starts
 * hiding anything. Group-control ops are always kept, so being added back
 * still applies. Chats that never name the device in a membership action — a
 * direct chat, or a group whose `Create` has not synced yet — are untouched.
 */
export function dropOpsAuthoredWhileNotAMember(
	opsOrdered: SimplifiedOperation<Payload>[],
	myDeviceId: DeviceId,
): SimplifiedOperation<Payload>[] {
	const windows = membershipWindows(opsOrdered, myDeviceId);
	if (windows.length === 0) return opsOrdered;
	return opsOrdered.filter(op => {
		if (op.body?.type !== 'Chat') return true;
		return stateAt(windows, op.header.timestamp, true);
	});
}

/** When `myDeviceId` joined and left this chat, from the group-control ops in
 * its log, sorted by ascending timestamp. */
function membershipWindows(
	opsOrdered: SimplifiedOperation<Payload>[],
	myDeviceId: DeviceId,
): StateChange[] {
	const windows: StateChange[] = [];
	for (const op of opsOrdered) {
		const action = op.header.auth?.action;
		if (!action) continue;
		const timestamp = op.header.timestamp;
		if ('Create' in action) {
			const included = action.Create.initial_members.some(([member]) =>
				isMe(member, myDeviceId),
			);
			if (included) windows.push({ value: true, timestamp });
		} else if ('Add' in action && isMe(action.Add.member, myDeviceId)) {
			windows.push({ value: true, timestamp });
		} else if ('Remove' in action && isMe(action.Remove.member, myDeviceId)) {
			windows.push({ value: false, timestamp });
		}
	}
	return windows;
}

function isMe(member: AuthGroupMember, myDeviceId: DeviceId): boolean {
	return 'Individual' in member && member.Individual === myDeviceId;
}
