<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { Card, StatusPill, ProgressBar, Icon } from '$lib/ui';
	import { getJob, streamJob } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Job, JobState } from '$lib/api/types';
	import { ArrowLeft01Icon } from '@hugeicons/core-free-icons';

	const id = $derived(page.params.id!);
	let job = $state<Job | null>(null);
	let error = $state<string | null>(null);
	let stop: (() => void) | undefined;
	let format = $state<'hls' | 'dash'>('hls');

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];

	// Attach the right player when the <video> mounts. Keyed on {format} so
	// switching HLS/DASH remounts and re-attaches cleanly.
	function player(node: HTMLVideoElement, opts: { format: 'hls' | 'dash'; src: string }) {
		let instance: { destroy(): void } | undefined;
		(async () => {
			if (opts.format === 'dash') {
				const { MediaPlayer } = await import('dashjs');
				const p = MediaPlayer().create();
				p.initialize(node, opts.src, false);
				instance = { destroy: () => p.destroy() };
				return;
			}
			// HLS: prefer hls.js (MSE) — Chrome falsely reports canPlayType='maybe'
			// but can't play HLS natively; only Safari truly can.
			const { default: Hls } = await import('hls.js');
			if (Hls.isSupported()) {
				const hls = new Hls();
				hls.loadSource(opts.src);
				hls.attachMedia(node);
				instance = hls;
			} else if (node.canPlayType('application/vnd.apple.mpegurl')) {
				node.src = opts.src;
			}
		})();
		return { destroy: () => instance?.destroy() };
	}

	onMount(async () => {
		try {
			job = await getJob(id); // initial snapshot (also fills playback URLs if done)
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load job.';
			return;
		}
		// Live updates until terminal; the stream closes itself when done.
		if (ACTIVE.includes(job.state)) {
			stop = streamJob(id, (updated) => (job = updated));
		}
	});
	onDestroy(() => stop?.());
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
			{@const src = format === 'dash' ? job.dash_url! : job.playback_url}
			<Card class="overflow-hidden !p-0">
				{#key format}
					<!-- svelte-ignore a11y_media_has_caption -->
					<video
						use:player={{ format, src }}
						controls
						playsinline
						poster={job.poster_url}
						class="aspect-video w-full bg-black"
					></video>
				{/key}
			</Card>
			{#if job.dash_url}
				<div class="mono mt-3 inline-flex overflow-hidden rounded-lg border border-border text-xs">
					<button
						class={`px-3 py-1 ${format === 'hls' ? 'bg-accent text-accent-fg' : 'text-muted hover:text-fg'}`}
						onclick={() => (format = 'hls')}>HLS</button
					>
					<button
						class={`px-3 py-1 ${format === 'dash' ? 'bg-accent text-accent-fg' : 'text-muted hover:text-fg'}`}
						onclick={() => (format = 'dash')}>DASH</button
					>
				</div>
			{/if}
			<p class="mono mt-3 break-all text-xs text-muted">{src}</p>
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
