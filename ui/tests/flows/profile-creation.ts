import { S } from '../selectors';

/**
 * Profile creation flow.
 *
 * Precondition: App is on the create-profile screen (first launch).
 *
 * Steps (Material theme):
 *   1. Wait for selector: S.createProfile.nameInput
 *   2. Type name into: S.createProfile.nameInput + ' input'
 *   3. Optionally type surname into: S.createProfile.surnameInput + ' input'
 *   4. Click: S.createProfile.createButton
 *   5. Wait for selector: S.home.chatList  (redirects to home after creation)
 *
 * Steps (iOS theme):
 *   Same as above but step 4 uses: S.createProfile.createLink
 */

export const steps = {
	waitForForm: S.createProfile.nameInput,
	nameInput: `${S.createProfile.nameInput} input`,
	surnameInput: `${S.createProfile.surnameInput} input`,
	createButton: S.createProfile.createButton,
	createLink: S.createProfile.createLink,
	successIndicator: S.home.chatList,
};
