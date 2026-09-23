/** Moves the cloud mailbox makes: its link turning slow, hanging, refusing
 *  connections, the server itself stopping, and everything healing. They only apply to a run that models the
 *  cloud; elsewhere their `check` is false and they are skipped. Each ends
 *  by asserting what every driveable agent's chip shows, so a sequence
 *  fails at the exact move the chip stopped telling the truth. */
import {
	cutMailboxLink,
	hangMailboxLink,
	healMailboxLink,
	resumeMailbox,
	slowMailboxLink,
	suspendMailbox,
} from '../../../setup/mailbox-control';
import {
	MAILBOX_HEALED_MS,
	MAILBOX_HUNG_MS,
	MAILBOX_UNANSWERED_MS,
} from '../../timeouts';
import { type Real, byName, log } from '../agents';
import { checkCloud } from '../checks';
import type { ExpectedModel } from '../model';
import { Move, type Moves } from './move';

/** Every driveable agent checks its chip. */
async function checkCloudAll(
	m: ExpectedModel,
	real: Real,
	after: string,
	within: number,
): Promise<void> {
	for (const name of m.activeNames()) {
		await checkCloud(m, byName(real, name), after, within);
	}
}

/** Slow the link down. The cloud stays usable, so the chip stays hidden. */
class CloudSlowMove extends Move {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.hasCloud() && m.cloudUsable();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		await slowMailboxLink();
		await checkCloudAll(
			m,
			real,
			'the cloud link turned slow',
			MAILBOX_HEALED_MS,
		);
	}

	toString(): string {
		return 'cloudSlow()';
	}
}

/** Make the link unusable, hanging every request or refusing every
 *  connection. Chips must read disconnected. */
class CloudDropMove extends Move {
	constructor(readonly how: 'hang' | 'cut' | 'suspend') {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.hasCloud() && m.cloudUsable();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		if (this.how === 'hang') await hangMailboxLink();
		else if (this.how === 'cut') await cutMailboxLink();
		else suspendMailbox();
		m.setCloudUsable(false);
		await checkCloudAll(
			m,
			real,
			DROPPED[this.how].what,
			this.how === 'hang' ? MAILBOX_HUNG_MS : MAILBOX_UNANSWERED_MS,
		);
	}

	toString(): string {
		return DROPPED[this.how].name;
	}
}

/** What each way of dropping the cloud is called, and how a report names it. */
const DROPPED = {
	hang: { name: 'cloudHang()', what: 'the cloud link hung' },
	cut: { name: 'cloudCut()', what: 'the cloud link was cut' },
	suspend: { name: 'cloudStop()', what: 'the cloud server stopped' },
} as const;

/** Bring the cloud back, whichever way it went.
 *
 * Healing the link and resuming the server are both unconditional because a
 * move only records that the cloud is unusable, not which way it was made so —
 * and either undone on a cloud that never had it done is a no-op. */
class CloudHealMove extends Move {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.hasCloud() && !m.cloudUsable();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		resumeMailbox();
		await healMailboxLink();
		m.setCloudUsable(true);
		await checkCloudAll(m, real, 'the cloud link healed', MAILBOX_HEALED_MS);
	}

	toString(): string {
		return 'cloudHeal()';
	}
}

export const cloudMoves: Moves = [
	{ build: () => new CloudSlowMove(), weight: 1 },
	{ build: () => new CloudDropMove('hang'), weight: 2 },
	{ build: () => new CloudDropMove('cut'), weight: 2 },
	// Stopping the server withholds the blob bytes as well as the operations:
	// media travels over the mailbox's iroh endpoint, which a link degraded in
	// front of its HTTP port leaves running.
	{ build: () => new CloudDropMove('suspend'), weight: 2 },
	{ build: () => new CloudHealMove(), weight: 4 },
];

/** The cloud moves by the names a search prints them under. */
export const move = {
	cloudSlow: (): Move => new CloudSlowMove(),
	cloudHang: (): Move => new CloudDropMove('hang'),
	cloudCut: (): Move => new CloudDropMove('cut'),
	cloudStop: (): Move => new CloudDropMove('suspend'),
	cloudHeal: (): Move => new CloudHealMove(),
};
