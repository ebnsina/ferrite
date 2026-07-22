<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { Card, StatusPill, ProgressBar, Icon } from '$lib/ui';
	import { getJob } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Job, JobState } from '$lib/api/types';
	import { ArrowLeft01Icon } from '@hugeicons/core-free-icons';

	const id = $derived(page.params.id!);
	let job = $state<Job | null>(null);
	let error = $state<string | null>(null);
	let timer: ReturnType<typeof setInterval>;

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];

	async function load() {
		try {
			job = await getJob(id);
			error = null;
			if (!ACTIVE.includes(job.state)) clearInterval(timer);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load job.';
			clearInterval(timer);
		}
	}

	// Svelte action: attach HLS when the <video> mounts. Prefer hls.js (MSE) —
	// Chrome falsely reports canPlayType('…mpegurl')='maybe' but can't play HLS
	// natively; only Safari truly can, so native is the fallback.
	function hlsPlayer(node: HTMLVideoElement, src: string) {
		let instance: { destroy(): void } | undefined;
		(async () => {
			const { default: Hls } = await import('hls.js');
			if (Hls.isSupported()) {
				const hls = new Hls();
				hls.on(Hls.Events.ERROR, (_e, data) =>
					console.error('[hls]', data.type, data.details, data.fatal)
				);
				hls.loadSource(src);
				hls.attachMedia(node);
				instance = hls;
			} else if (node.canPlayType('application/vnd.apple.mpegurl')) {
				node.src = src; // Safari native HLS
			}
		})();
		return { destroy: () => instance?.destroy() };
	}

	onMount(() => {
		load();
		timer = setInterval(load, 2000);
	});
	onDestroy(() => clearInterval(timer));
</script>

<div class="mx-auto max-w-3xl">
	<a href="/jobs" class="mb-6 inline-flex items-center gap-1 text-sm text-muted hover:text-fg">
		<Icon icon={ArrowLeft01Icon} size={16} /> Jobs
	</a>

	{#if error}
		<div class="rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{:else if !job}
		<p class="text-sm text-muted">Loading…</p>
	{:else}
		<div class="mb-4 flex items-center justify-between">
			<code class="mono text-sm text-muted">{job.id}</code>
			<StatusPill state={job.state} />
		</div>

		{#if job.playback_url}
			<Card class="overflow-hidden !p-0">
				<!-- svelte-ignore a11y_media_has_caption -->
				<video
					use:hlsPlayer={job.playback_url}
					controls
					playsinline
					poster={job.poster_url}
					class="aspect-video w-full bg-black"
				></video>
			</Card>
			<p class="mono mt-3 break-all text-xs text-muted">{job.playback_url}</p>
		{:else if ACTIVE.includes(job.state)}
			<Card>
				<p class="mb-3 text-sm text-muted">Transcoding…</p>
				<ProgressBar value={job.progress} />
			</Card>
		{:else if job.error}
			<Card><p class="text-sm text-danger">{job.error}</p></Card>
		{/if}
	{/if}
</div>
