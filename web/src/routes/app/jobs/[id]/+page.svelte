<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { Card, StatusPill, ProgressBar, Icon } from '$lib/ui';
	import { getJob, streamJob } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Job, JobState } from '$lib/api/types';
	import { ArrowLeft01Icon, HdIcon, ArrowDown01Icon } from '@hugeicons/core-free-icons';

	const id = $derived(page.params.id!);
	let job = $state<Job | null>(null);
	let error = $state<string | null>(null);
	let stop: (() => void) | undefined;
	let format = $state<'hls' | 'dash'>('hls');

	// Rendition (quality) selection — driven by hls.js levels.
	interface Rendition {
		i: number;
		label: string;
		bitrate: number;
	}
	let levels = $state<Rendition[]>([]);
	let selected = $state(-1); // -1 = auto
	let activeLevel = $state(-1); // level hls.js is actually playing (for auto label)
	let menuOpen = $state(false);
	let hlsRef: import('hls.js').default | null = null;

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];

	function resetRenditions() {
		levels = [];
		selected = -1;
		activeLevel = -1;
		menuOpen = false;
		hlsRef = null;
	}

	function selectQuality(i: number) {
		if (hlsRef) {
			selected = i;
			hlsRef.currentLevel = i; // -1 lets hls.js resume auto ABR
		}
		menuOpen = false;
	}

	const currentLabel = $derived(
		selected >= 0
			? (levels.find((l) => l.i === selected)?.label ?? 'Auto')
			: activeLevel >= 0 && levels.find((l) => l.i === activeLevel)
				? `Auto · ${levels.find((l) => l.i === activeLevel)!.label}`
				: 'Auto'
	);

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
				hls.on(Hls.Events.MANIFEST_PARSED, () => {
					// Descending by bitrate so the menu reads best→worst.
					levels = hls.levels
						.map((l, i) => ({
							i,
							bitrate: l.bitrate,
							label: l.height ? `${l.height}p` : `${Math.round(l.bitrate / 1000)}k`
						}))
						.sort((a, b) => b.bitrate - a.bitrate);
					activeLevel = hls.currentLevel;
				});
				hls.on(Hls.Events.LEVEL_SWITCHED, (_e, data) => (activeLevel = data.level));
				hlsRef = hls;
				instance = hls;
			} else if (node.canPlayType('application/vnd.apple.mpegurl')) {
				node.src = opts.src; // Safari native HLS (no level API)
			}
		})();
		return {
			destroy: () => {
				instance?.destroy();
				resetRenditions();
			}
		};
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
	<a href="/app/jobs" class="mb-6 inline-flex items-center gap-1 text-sm text-muted hover:text-fg">
		<Icon icon={ArrowLeft01Icon} size={16} /> Jobs
	</a>

	{#if error}
		<div class="rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
			{error}
		</div>
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

			<div class="mt-3 flex flex-wrap items-center gap-3">
				{#if job.dash_url}
					<div class="mono inline-flex overflow-hidden rounded-lg border border-border text-xs">
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

				{#if format === 'hls' && levels.length > 1}
					<div class="relative">
						<button
							onclick={() => (menuOpen = !menuOpen)}
							class="mono inline-flex items-center gap-1.5 rounded-lg border border-border px-3 py-1 text-xs text-muted hover:text-fg"
						>
							<Icon icon={HdIcon} size={14} />
							{currentLabel}
							<Icon icon={ArrowDown01Icon} size={12} />
						</button>
						{#if menuOpen}
							<div
								class="absolute bottom-full left-0 z-10 mb-1 min-w-32 overflow-hidden rounded-lg border border-border bg-surface shadow-lg"
							>
								<button
									onclick={() => selectQuality(-1)}
									class={`mono block w-full px-3 py-1.5 text-left text-xs hover:bg-surface-2 ${selected === -1 ? 'text-accent' : 'text-muted'}`}
									>Auto</button
								>
								{#each levels as l (l.i)}
									<button
										onclick={() => selectQuality(l.i)}
										class={`mono block w-full px-3 py-1.5 text-left text-xs hover:bg-surface-2 ${selected === l.i ? 'text-accent' : 'text-fg'}`}
										>{l.label}</button
									>
								{/each}
							</div>
						{/if}
					</div>
				{/if}
			</div>

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
