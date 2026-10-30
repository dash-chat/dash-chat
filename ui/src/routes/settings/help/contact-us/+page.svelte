<script lang="ts">
	import { goBack } from '$lib/utils/navigation';
	import { m } from '$lib/paraglide/messages.js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import ScreenshotsInput from '$lib/components/ScreenshotsInput.svelte';
	import { sendFeedback, type FeedbackReason } from '$lib/utils/error-report';
	import { showToast } from '$lib/utils/toasts';
	import {
		Block,
		BlockTitle,
		Checkbox,
		Link,
		List,
		ListInput,
		ListItem,
		Navbar,
		NavbarBackLink,
		Page,
		useTheme,
	} from 'konsta/svelte';
	import FixedActionButton from '$lib/components/FixedActionButton.svelte';
	import { isIos } from '$lib/utils/environment';

	const theme = $derived(useTheme());

	// Without SENTRY_DSN there is nowhere for a submission to go.
	const canSend = import.meta.env.VITE_SENTRY_ENABLED;
	const MAX_SCREENSHOTS = 5;
	const MAX_SCREENSHOT_BYTES = 8 * 1024 * 1024;

	let message = $state('');
	let reason = $state<FeedbackReason | undefined>();
	let includeDebugLog = $state(true);
	let screenshots = $state<File[]>([]);
	let attachingScreenshots = $state(false);
	let sending = $state(false);
	const sendDisabled = $derived(
		!message || !reason || !canSend || sending || attachingScreenshots,
	);

	async function handleSubmit() {
		if (!reason || sendDisabled) return;
		sending = true;
		try {
			const outcome = await sendFeedback({
				reason,
				message,
				screenshots,
				includeLogs: includeDebugLog,
			});
			showToast(outcome === 'queued' ? m.reportQueued() : m.reportSent());
			goBack('/settings/help');
		} catch (e) {
			console.error('Error sending feedback', e);
			showToast(m.errorUnexpected(), 'unexpected', e);
		} finally {
			sending = false;
		}
	}
</script>

<Page>
	<Navbar
		title={m.contactUs()}
		titleClass="opacity1"
		transparent={true}
		rightClass={sendDisabled ? 'ios-right-disabled' : ''}
	>
		{#snippet left()}
			<NavbarBackLink
				onClick={() => goBack('/settings/help')}
				data-testid="contact-us-back"
			/>
		{/snippet}

		{#snippet right()}
			{#if isIos}
				<Link onClick={handleSubmit} data-testid="contact-us-send-btn">
					{m.send()}
				</Link>
			{/if}
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
					<option value={undefined} disabled>
						{m.pleaseSelectAnOption()}
					</option>
					<option value="bug">{m.reasonBugReport()}</option>
					<option value="feature">{m.reasonFeatureRequest()}</option>
					<option value="question">{m.reasonQuestion()}</option>
					<option value="feedback">{m.reasonGeneralFeedback()}</option>
					<option value="other">{m.reasonOther()}</option>
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
					maxlength={4096}
					data-testid="contact-us-message-input"
				/>
			</List>

			<BlockTitle id="contact-us-screenshots-title"
				>{m.attachScreenshots()}</BlockTitle
			>
			<Block
				strongIos
				inset={isWideScreen.value || theme === 'ios'}
				class="!mb-0"
			>
				<ScreenshotsInput
					bind:value={screenshots}
					bind:attaching={attachingScreenshots}
					max={MAX_SCREENSHOTS}
					maxBytes={MAX_SCREENSHOT_BYTES}
					disabled={sending}
					labelledBy="contact-us-screenshots-title"
					testId="contact-us-screenshots"
				/>
			</Block>

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

			{#if !canSend}
				<p class="px-4 pt-4 text-sm opacity-60">{m.feedbackUnavailable()}</p>
			{/if}
		</div>
	</div>

	{#if !isIos}
		<FixedActionButton
			onClick={handleSubmit}
			disabled={sendDisabled}
			loading={sending}
			testId="contact-us-send-btn"
		>
			{m.send()}
		</FixedActionButton>
	{/if}
</Page>
