<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Card, StatusPill, ProgressBar } from '$lib/ui';
	import { listJobs, getJob } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Job, JobState } from '$lib/api/types';
	import { timeAgo } from '$lib/format';
	import { ListVideo, Play } from '@lucide/svelte';

	type JobRow = Job & { playback_url?: string };

	let jobs = $state<JobRow[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let timer: ReturnType<typeof setInterval>;

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];
	const hasActive = $derived(jobs.some((j) => ACTIVE.includes(j.state)));

	async function load() {
		try {
			jobs = await listJobs();
			error = null;
			// Enrich completed jobs with a playback URL.
			await Promise.all(
				jobs.map(async (j, i) => {
					if (j.state === 'completed' && !j.playback_url) {
						jobs[i] = await getJob(j.id);
					}
				})
			);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load jobs.';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		load();
		timer = setInterval(() => hasActive && load(), 2000);
	});
	onDestroy(() => clearInterval(timer));
</script>

<div class="mx-auto max-w-5xl">
	<div class="mb-8">
		<h1 class="text-2xl font-semibold tracking-tight">Jobs</h1>
		<p class="mt-1 text-sm text-muted">Transcode jobs and their status.</p>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<Card>
		{#if loading}
			<p class="py-8 text-center text-sm text-muted">Loading…</p>
		{:else if jobs.length === 0}
			<div class="flex flex-col items-center justify-center py-12 text-center">
				<span class="mb-3 text-muted"><ListVideo size={32} /></span>
				<p class="font-medium">No jobs yet</p>
				<p class="mt-1 text-sm text-muted">Start a transcode from the Assets page.</p>
			</div>
		{:else}
			<div class="flex flex-col divide-y divide-border">
				{#each jobs as job (job.id)}
					<div class="flex items-center gap-4 py-3">
						<code class="mono w-20 shrink-0 truncate text-xs text-muted">{job.id.slice(0, 8)}</code>
						<div class="min-w-0 flex-1">
							{#if ACTIVE.includes(job.state)}
								<ProgressBar value={job.progress} />
							{:else if job.error}
								<p class="truncate text-sm text-danger">{job.error}</p>
							{:else}
								<p class="text-xs text-muted">{timeAgo(job.queued_at)}</p>
							{/if}
						</div>
						{#if job.playback_url}
							<a
								href={job.playback_url}
								target="_blank"
								rel="noreferrer"
								class="flex items-center gap-1 text-xs text-accent hover:underline"
							>
								<Play size={14} /> Playlist
							</a>
						{/if}
						<StatusPill state={job.state} />
					</div>
				{/each}
			</div>
		{/if}
	</Card>
</div>
