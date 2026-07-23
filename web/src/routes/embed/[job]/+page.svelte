<script lang="ts">
	import { page } from '$app/state';
	import { API_BASE } from '$lib/api/client';
	import VideoPlayer from '$lib/components/VideoPlayer.svelte';

	const job = $derived(page.params.job!);
	const token = $derived(page.url.searchParams.get('token') ?? '');
	const hasCaptions = $derived(page.url.searchParams.get('cc') === '1');

	const src = $derived(`${API_BASE}/playback/${job}/master.m3u8?token=${token}`);
	const poster = $derived(`${API_BASE}/playback/${job}/thumbs/poster.jpg?token=${token}`);
	const captionsUrl = $derived(
		hasCaptions ? `${API_BASE}/playback/${job}/captions.vtt?token=${token}` : undefined
	);
	const analytics = $derived({ beaconUrl: `${API_BASE}/playback/beacon`, job, token });
</script>

<svelte:head>
	<title>Ferrite player</title>
	<meta name="robots" content="noindex" />
</svelte:head>

<div class="h-screen w-screen bg-black">
	{#if token}
		<VideoPlayer {src} {poster} {captionsUrl} {analytics} class="h-full w-full" />
	{:else}
		<div class="flex h-full items-center justify-center text-sm text-muted">
			Missing or invalid embed token.
		</div>
	{/if}
</div>
