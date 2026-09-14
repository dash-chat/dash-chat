/** Moves the cloud mailbox's link makes: turning slow, hanging, refusing
 *  connections, and healing. They only apply to a run that models the
 *  cloud; elsewhere their `check` is false and they are skipped. Each ends
 *  by asserting what every driveable agent's chip shows, so a sequence
 *  fails at the exact move the chip stopped telling the truth. */
import fc from 'fast-check';

import type { Link } from '../../../setup/toxiproxy';
import { MAILBOX_HEALED_MS, MAILBOX_UNANSWERED_MS } from '../../timeouts';
import { type Real, byName, log } from '../agents';
import { checkCloud } from '../checks';
import type { ExpectedModel } from '../model';
import { Move, type Moves } from './move';

function cloudLink(real: Real): Link {
	if (real.cloud === null) throw new Error('the run has no cloud mailbox');
	return real.cloud;
}

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
		return m.cloudUsable();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		await cloudLink(real).slow();
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
	constructor(readonly how: 'hang' | 'cut') {
		super();
	}

	check(m: Readonly<ExpectedModel>): boolean {
		return m.cloudUsable();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		const link = cloudLink(real);
		if (this.how === 'hang') await link.hang();
		else await link.cut();
		m.setCloudUsable(false);
		await checkCloudAll(
			m,
			real,
			this.how === 'hang' ? 'the cloud link hung' : 'the cloud link was cut',
			MAILBOX_UNANSWERED_MS,
		);
	}

	toString(): string {
		return this.how === 'hang' ? 'cloudHang()' : 'cloudCut()';
	}
}

/** Heal the link. Chips must hide again, and whatever the cloud held
 *  reaches everyone. */
class CloudHealMove extends Move {
	check(m: Readonly<ExpectedModel>): boolean {
		return m.hasCloud() && !m.cloudUsable();
	}

	async perform(m: ExpectedModel, real: Real): Promise<void> {
		log(this.toString());
		await cloudLink(real).heal();
		m.setCloudUsable(true);
		await checkCloudAll(m, real, 'the cloud link healed', MAILBOX_HEALED_MS);
	}

	toString(): string {
		return 'cloudHeal()';
	}
}

export const cloudMoves: Moves = [
	{ arbitrary: fc.constant(new CloudSlowMove()), weight: 1 },
	{ arbitrary: fc.constant(new CloudDropMove('hang')), weight: 2 },
	{ arbitrary: fc.constant(new CloudDropMove('cut')), weight: 2 },
	{ arbitrary: fc.constant(new CloudHealMove()), weight: 4 },
];

/** The cloud moves by the names a search prints them under. */
export const move = {
	cloudSlow: (): Move => new CloudSlowMove(),
	cloudHang: (): Move => new CloudDropMove('hang'),
	cloudCut: (): Move => new CloudDropMove('cut'),
	cloudHeal: (): Move => new CloudHealMove(),
};
