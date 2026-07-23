<script lang="ts">
	import { Card, Icon } from '$lib/ui';
	import { searchVideos, type SearchHit } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { Search01Icon, PlayCircleIcon } from '@hugeicons/core-free-icons';

	let q = $state('');
	let hits = $state<SearchHit[]>([]);
	let searched = $state(false);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let timer: ReturnType<typeof setTimeout>;

	function timecode(secs: number): string {
		const s = Math.floor(secs);
		return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
	}

	async function run() {
		const term = q.trim();
		if (!term) {
			hits = [];
			searched = false;
			return;
		}
		loading = true;
		error = null;
		try {
			hits = await searchVideos(term);
			searched = true;
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Search failed.';
		} finally {
			loading = false;
		}
	}

	function onInput() {
		clearTimeout(timer);
		timer = setTimeout(run, 250); // debounce
	}
</script>

<div class="mx-auto max-w-3xl">
	<div class="mb-6">
		<h1 class="text-2xl font-semibold tracking-tight">Search inside your videos</h1>
		<p class="mt-1 text-sm text-muted">
			Search the spoken words across every captioned video — jump straight to the moment.
		</p>
	</div>

	<div class="relative mb-6">
		<span class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted">
			<Icon icon={Search01Icon} size={18} />
		</span>
		<input
			bind:value={q}
			oninput={onInput}
			placeholder="e.g. pricing, onboarding, refund policy…"
			class="w-full rounded-xl border border-border bg-surface-2 py-3 pr-4 pl-10 text-sm outline-none focus:border-accent"
		/>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
			{error}
		</div>
	{/if}

	{#if loading}
		<p class="py-8 text-center text-sm text-muted">Searching…</p>
	{:else if searched && hits.length === 0}
		<Card>
			<div class="py-8 text-center">
				<p class="text-sm font-medium">No matches</p>
				<p class="mt-1 text-xs text-muted">
					Only videos transcoded with auto-captions are searchable.
				</p>
			</div>
		</Card>
	{:else if hits.length > 0}
		<div class="flex flex-col gap-2">
			{#each hits as h (h.job_id + h.start_secs)}
				<a
					href={`/app/jobs/${h.job_id}?t=${Math.floor(h.start_secs)}`}
					class="group flex items-center gap-4 rounded-xl border border-border bg-surface p-4 transition-colors hover:border-accent/40 hover:bg-surface-2"
				>
					<span class="text-muted group-hover:text-accent"><Icon icon={PlayCircleIcon} size={22} /></span>
					<div class="min-w-0 flex-1">
						<p class="truncate text-sm">“{h.snippet}”</p>
						<p class="mt-1 truncate text-xs text-muted">{h.filename}</p>
					</div>
					<span class="mono shrink-0 rounded-md bg-surface-2 px-2 py-1 text-xs text-muted"
						>{timecode(h.start_secs)}</span
					>
				</a>
			{/each}
		</div>
	{:else}
		<Card>
			<p class="py-8 text-center text-sm text-muted">
				Type to search across your video transcripts.
			</p>
		</Card>
	{/if}
</div>
