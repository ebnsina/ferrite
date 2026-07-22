<script lang="ts">
	import type { JobState } from '$lib/api/types';

	interface Props {
		state: JobState;
	}

	let { state }: Props = $props();

	// Map each job state to a tone. In-flight states share the accent tone.
	const tones: Record<JobState, string> = {
		queued: 'bg-surface-2 text-muted border-border',
		probing: 'bg-accent-soft text-accent border-accent/30',
		transcoding: 'bg-accent-soft text-accent border-accent/30',
		packaging: 'bg-accent-soft text-accent border-accent/30',
		uploading: 'bg-accent-soft text-accent border-accent/30',
		completed: 'bg-success/10 text-success border-success/30',
		failed: 'bg-danger/10 text-danger border-danger/30'
	};
</script>

<span
	class={`mono inline-flex items-center rounded-full border px-2 py-0.5 text-xs font-medium ${tones[state]}`}
>
	{state}
</span>
