<script lang="ts">
	export let text: string;
</script>

<div class="test-banner z-50 px-4 py-1 text-center text-md font-medium">
	<span class="test-banner-inner">{text}</span>
</div>

<style>
	.test-banner {
		--banner-stripe-yellow: #f9ecb0;
		--banner-stripe-dark: #bbbbbb;
		background: repeating-linear-gradient(
			45deg,
			var(--banner-stripe-yellow) 0 16px,
			var(--banner-stripe-dark) 16px 32px
		);
		border-color: var(--banner-stripe-dark);
		border-bottom-width: 2px;
		text-transform: uppercase;
		color: #111;
		font-weight: bold;
		letter-spacing: 0.15em;
		word-spacing: 0.3em;
		margin-bottom: calc(20px - env(safe-area-inset-bottom, 0px));
	}
	:global(.dark) .test-banner {
		--banner-stripe-yellow: #b08100;
		--banner-stripe-dark: #666666;
		color: #fff;
	}

	.test-banner-inner {
		display: block;
		padding-top: env(safe-area-inset-top, 0px);
	}
</style>
