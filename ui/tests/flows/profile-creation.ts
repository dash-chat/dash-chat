import { S } from '../selectors';
import { waitFor, typeInto, click, nextTick } from '../helpers';

/**
 * Profile creation flow.
 *
 * Precondition: App is on first launch (onboarding carousel or create-profile screen).
 *
 * Steps:
 *   1. Dismiss onboarding carousel if visible (click Next through pages, then Start)
 *   2. Wait for selector: S.createProfile.nameInput
 *   3. Type name into: S.createProfile.nameInput + ' input'
 *   4. Optionally type surname into: S.createProfile.surnameInput + ' input'
 *   5. Click: S.createProfile.createButton
 *   6. Wait for selector: S.home.emptyState  (redirects to home after creation; no chats yet)
 *
 * Steps (iOS theme):
 *   Same as above but step 5 uses: S.createProfile.createLink
 */

export const steps = {
	waitForForm: S.createProfile.nameInput,
	nameInput: `${S.createProfile.nameInput} input`,
	surnameInput: `${S.createProfile.surnameInput} input`,
	createButton: S.createProfile.createButton,
	createLink: S.createProfile.createLink,
	successIndicator: S.home.emptyState,
};

/** Dismiss the onboarding carousel by clicking through all pages. */
async function dismissOnboarding(): Promise<void> {
	const nextBtn = document.querySelector(S.onboarding.nextButton) as HTMLElement | null;
	if (!nextBtn) return; // Not on onboarding screen

	// Click Next through pages until Start button appears
	while (document.querySelector(S.onboarding.nextButton)) {
		click(S.onboarding.nextButton);
		await nextTick();
	}

	// Click Start app
	click(S.onboarding.startButton);
	await waitFor(steps.waitForForm);
}

/** Create a profile and wait for the home page. */
export async function createProfile(name: string, surname: string): Promise<true> {
	// Dismiss onboarding if it's showing (first launch)
	const onboardingVisible = !!document.querySelector(S.onboarding.nextButton);
	if (onboardingVisible) {
		await dismissOnboarding();
	} else {
		await waitFor(steps.waitForForm);
	}

	typeInto(steps.nameInput, name);
	typeInto(steps.surnameInput, surname);
	await nextTick();
	click(steps.createButton);
	await waitFor(steps.successIndicator);
	return true;
}
