<script lang="ts">
	// Global root layout: loads styles for both the marketing site and the app.
	// Deliberately markup-free so each section owns its own chrome.
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { Toaster } from '$lib/ui';
	import { SITE } from '$lib/site';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<link rel="apple-touch-icon" href="/apple-touch-icon.png" />
	<meta name="theme-color" media="(prefers-color-scheme: dark)" content="#0b0d10" />
	<meta name="theme-color" media="(prefers-color-scheme: light)" content="#f7f8fa" />
	<!-- Social defaults; pages override title/description/url. -->
	<meta property="og:site_name" content={SITE.name} />
	<meta property="og:type" content="website" />
	<meta property="og:image" content={SITE.ogImage} />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta name="twitter:card" content="summary_large_image" />
	<meta name="twitter:image" content={SITE.ogImage} />
</svelte:head>

{@render children()}
<Toaster />
