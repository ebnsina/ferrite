<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { Card, Button, Icon, StatusPill, toast } from '$lib/ui';
	import VideoPlayer from '$lib/components/VideoPlayer.svelte';
	import { getAsset, getJob, listJobs, createJob, makeShorts } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import type { Asset, Job, JobState } from '$lib/api/types';
	import { bytes, timeAgo } from '$lib/format';
	import { ArrowLeft, Play, MagicWand, PlayCircle, ArrowRight } from 'phosphor-svelte';

	const id = $derived(page.params.id!);
	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];

	let asset = $state<Asset | null>(null);
	let jobs = $state<Job[]>([]);
	let playable = $state<Job | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let working = $state(false);

	async function load() {
		try {
			asset = await getAsset(id);
			const all = await listJobs();
			jobs = all
				.filter((j) => j.asset_id === id)
				.sort((a, b) => new Date(b.queued_at).getTime() - new Date(a.queued_at).getTime());
			// Find a completed transcode with a playable stream (newest first).
			playable = null;
			for (const j of jobs.filter((j) => j.state === 'completed')) {
				const full = await getJob(j.id);
				if (full.playback_url) {
					playable = full;
					break;
				}
			}
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load this video.');
		} finally {
			loading = false;
		}
	}
	onMount(load);

	async function transcode() {
		working = true;
		try {
			await createJob(id, {});
			toast.success('Video queued for processing.');
			goto('/app/jobs');
		} catch (e) {
			toast.error(humanizeError(e instanceof ApiError ? e.message : null, 'Could not start processing.'));
			working = false;
		}
	}

	async function shorts() {
		working = true;
		try {
			await makeShorts(id, 3);
			toast.success('Creating your shorts — this can take a few minutes.');
			goto('/app/jobs');
		} catch (e) {
			toast.error(humanizeError(e instanceof ApiError ? e.message : null, 'Could not start.'));
			working = false;
		}
	}
</script>

<div class="mx-auto max-w-3xl">
	<a href="/app/assets" class="mb-6 inline-flex items-center gap-1 text-sm text-muted hover:text-fg">
		<Icon icon={ArrowLeft} size={16} /> Videos
	</a>

	{#if error}
		<div class="rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{:else if loading}
		<p class="text-sm text-muted">Loading…</p>
	{:else if asset}
		<div class="mb-4 flex flex-wrap items-start justify-between gap-3">
			<div class="min-w-0">
				<h1 class="truncate text-xl font-semibold tracking-tight">{asset.filename}</h1>
				<p class="mono mt-1 text-xs text-muted">
					{bytes(asset.bytes)} · uploaded {timeAgo(asset.created_at)} · {asset.status}
				</p>
			</div>
		</div>

		<!-- Player / transcode CTA -->
		{#if playable?.playback_url}
			<Card class="overflow-hidden !p-0">
				<VideoPlayer
					src={playable.playback_url}
					poster={playable.poster_url}
					captionsUrl={playable.captions_url}
					class="aspect-video w-full"
				/>
			</Card>
			<a
				href={`/app/jobs/${playable.id}`}
				class="mt-3 inline-flex items-center gap-1 text-xs text-muted transition-colors hover:text-fg"
			>
				Renditions, embed &amp; analytics <Icon icon={ArrowRight} size={13} />
			</a>
		{:else}
			<Card class="overflow-hidden !p-0">
				<div class="relative flex aspect-video w-full items-center justify-center bg-black">
					{#if asset.thumbnail_url}
						<img src={asset.thumbnail_url} alt="" class="absolute inset-0 h-full w-full object-cover opacity-40" />
					{/if}
					<div class="relative flex flex-col items-center text-center">
						<span class="mb-2 text-white/80"><Icon icon={PlayCircle} size={40} /></span>
						<p class="text-sm font-medium text-white">Not streamable yet</p>
						<p class="mt-1 max-w-xs text-xs text-white/70">
							Process this video into adaptive HLS to stream it in the player.
						</p>
					</div>
				</div>
			</Card>
		{/if}

		<!-- Actions -->
		<div class="mt-4 flex flex-wrap gap-2">
			<Button onclick={transcode} disabled={working}>
				<Icon icon={Play} size={16} /> {playable ? 'Transcode again' : 'Transcode to stream'}
			</Button>
			<Button variant="secondary" onclick={shorts} disabled={working}>
				<Icon icon={MagicWand} size={15} /> Make shorts
			</Button>
		</div>

		<!-- Related jobs -->
		<Card class="mt-6">
			<h2 class="mb-4 text-sm font-medium">Jobs from this video</h2>
			{#if jobs.length === 0}
				<p class="py-6 text-center text-sm text-muted">No jobs yet — transcode it to get started.</p>
			{:else}
				<div class="flex flex-col divide-y divide-border">
					{#each jobs as job (job.id)}
						<a
							href={`/app/jobs/${job.id}`}
							class="flex items-center gap-4 py-2.5 transition-colors hover:bg-surface-2"
						>
							<code class="mono w-20 shrink-0 truncate text-xs text-muted">{job.id.slice(0, 8)}</code>
							<span class="flex-1 text-xs text-muted">
								{ACTIVE.includes(job.state) ? `${Math.round(job.progress * 100)}%` : timeAgo(job.finished_at ?? job.queued_at)}
							</span>
							<StatusPill state={job.state} />
						</a>
					{/each}
				</div>
			{/if}
		</Card>
	{/if}
</div>
