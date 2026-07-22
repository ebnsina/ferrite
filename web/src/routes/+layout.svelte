<script lang="ts">
	// Global root layout: loads styles for both the marketing site and the app.
	// Deliberately markup-free so each section owns its own chrome.
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';

	let { children } = $props();
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

{@render children()}
