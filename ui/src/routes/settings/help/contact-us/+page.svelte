<script lang="ts">
	import { goto } from '$app/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import { isMobile } from '$lib/utils/environment';
	import { sendMailto } from '$lib/utils/mailto';
	import { showToast } from '$lib/utils/toasts';
	import {
		BlockTitle,
		Button,
		Checkbox,
		List,
		ListInput,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		useTheme,
	} from 'konsta/svelte';

	const theme = $derived(useTheme());

	let message = $state('');
	let reason = $state('');
	let includeDebugLog = $state(true);
	let screenshot = $state<File | null>(null);
	let screenshotFilePicker: HTMLInputElement;

	function onScreenshotSelected() {
		screenshot = screenshotFilePicker.files?.[0] ?? null;
	}

	const reasonLabels: Record<string, () => string> = {
		bug: () => m.reasonBugReport(),
		feature: () => m.reasonFeatureRequest(),
		question: () => m.reasonQuestion(),
		other: () => m.reasonOther(),
	};

	async function handleSubmit() {
		const subjectParts: string[] = [];
		if (reason) subjectParts.push(reasonLabels[reason]());
		const subject =
			subjectParts.length > 0
				? `Dash Chat: ${subjectParts.join(' - ')}`
				: 'Dash Chat';

		try {
			await sendMailto({
				subject,
				body: message,
				includeDebugLog,
				attachments: screenshot ? [screenshot] : undefined,
			});
			goto('/settings/help');
		} catch (e) {
			console.error('Error sending contact email', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		}
	}
</script>

<Page>
	<input
		type="file"
		accept="image/*"
		bind:this={screenshotFilePicker}
		class="hidden"
		onchange={onScreenshotSelected}
	/>
	<Navbar title={m.contactUs()} titleClass="opacity1" transparent={true}>
		{#snippet left()}
			<NavbarBackLink
				onClick={() => goto('/settings/help')}
				data-testid="contact-us-back"
			/>
		{/snippet}
	</Navbar>

	<div class="column" style="flex: 1">
		<div class="column center-in-desktop">
			<BlockTitle>{m.tellUsWhyReachingOut()}</BlockTitle>
			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mb-0"
			>
				<ListInput
					type="select"
					bind:value={reason}
					data-testid="contact-us-reason-select"
				>
					{#snippet input()}
						<select bind:value={reason}>
							<option value="" disabled>{m.pleaseSelectAnOption()}</option>
							<option value="bug">{m.reasonBugReport()}</option>
							<option value="feature">{m.reasonFeatureRequest()}</option>
							<option value="question">{m.reasonQuestion()}</option>
							<option value="feedback">{m.reasonGeneralFeedback()}</option>
							<option value="other">{m.reasonOther()}</option>
						</select>
					{/snippet}
				</ListInput>
			</List>

			<BlockTitle>{m.contactUs()}</BlockTitle>
			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mb-0"
			>
				<ListInput
					type="textarea"
					placeholder={m.tellUsWhatsGoingOn()}
					bind:value={message}
					inputClass="!h-32 resize-none"
					data-testid="contact-us-message-input"
				/>
			</List>

			<BlockTitle>{m.attachScreenshot()}</BlockTitle>
			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mb-0"
			>
				{#if screenshot}
					<ListItem
						title={screenshot.name}
						data-testid="contact-us-screenshot-item"
						link
						onClick={() => {
							screenshot = null;
							screenshotFilePicker.value = '';
						}}
					>
						{#snippet after()}
							<span class="text-red-500">{m.removeScreenshot()}</span>
						{/snippet}
					</ListItem>
				{:else}
					<ListItem
						title={isMobile
							? m.tapToAttachScreenshot()
							: m.chooseFileToAttach()}
						link
						data-testid="contact-us-screenshot-picker"
						onClick={() => screenshotFilePicker.click()}
					/>
				{/if}
			</List>

			<List
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mt-4 !mb-0"
			>
				<ListItem
					title={m.includeDebugLog()}
					data-testid="contact-us-include-debug-log"
					onClick={() => (includeDebugLog = !includeDebugLog)}
				>
					{#snippet media()}
						<Checkbox
							checked={includeDebugLog}
							onChange={() => (includeDebugLog = !includeDebugLog)}
						/>
					{/snippet}
					{#snippet after()}{/snippet}
				</ListItem>
			</List>
		</div>
	</div>

	<Button
		rounded
		onClick={handleSubmit}
		disabled={!message}
		data-testid="contact-us-next-btn"
		class="fixed-action-btn"
	>
		{m.next()}
	</Button>
</Page>
