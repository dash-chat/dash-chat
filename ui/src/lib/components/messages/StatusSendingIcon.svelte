<svg
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="2"
	stroke-linecap="round"
	aria-hidden="true"
>
	<circle class="spinner" cx="12" cy="12" r="9" stroke-dasharray="1 4" />
	<path d="M7.5 12h7" />
	<path d="M11.5 8.5 15 12l-3.5 3.5" stroke-linejoin="round" />
</svg>

<style>
	svg {
		width: 100%;
		height: 100%;
	}

	.spinner {
		transform-origin: center;
		animation: spin 2s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
