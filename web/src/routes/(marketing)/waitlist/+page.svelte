<script lang="ts">
	import { page } from '$app/state';
	import { Icon } from '$lib/ui';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { CheckCircle, ArrowRight } from 'phosphor-svelte';

	const planParam = $derived(page.url.searchParams.get('plan') ?? '');

	let name = $state('');
	let email = $state('');
	let whatsapp = $state('');
	let country = $state('Bangladesh');
	let useCase = $state('');
	let volume = $state('');
	let plan = $state('');
	let payment = $state('');
	let busy = $state(false);
	let done = $state(false);
	let error = $state<string | null>(null);

	// Prefill plan from the pricing CTA.
	$effect(() => {
		if (planParam && !plan) plan = planParam;
	});

	const volumes = [
		'Just exploring',
		'Under 100 videos / month',
		'100 – 1,000',
		'1,000 – 10,000',
		'10,000+'
	];
	const plans = ['Cloud — Starter', 'Cloud — Pro', 'Enterprise', 'Self-host'];
	const payments = ['bKash', 'Nagad', 'Card (Visa/Mastercard)', 'Bank transfer', 'Not sure yet'];

	async function submit() {
		if (!name.trim() || !email.includes('@')) {
			error = 'Please enter your name and a valid email.';
			return;
		}
		busy = true;
		error = null;
		try {
			const res = await fetch(`${PUBLIC_API_URL}/waitlist`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({
					name,
					email,
					whatsapp,
					country,
					use_case: useCase,
					volume,
					plan: plan || planParam,
					payment
				})
			});
			if (!res.ok) throw new Error();
			done = true;
		} catch {
			error = 'Something went wrong. Please try again.';
		} finally {
			busy = false;
		}
	}

	const inputCls =
		'w-full rounded-lg border border-border bg-surface-2 px-3 py-2.5 text-sm outline-none focus:border-accent';
</script>

<svelte:head>
	<title>Join the waitlist — Ferrite Stream</title>
	<meta
		name="description"
		content="Get early access to Ferrite Stream Cloud — managed adaptive video with a 14-day free trial. Tell us what you're building."
	/>
	<meta property="og:title" content="Join the Ferrite Stream early-access waitlist" />
	<meta
		property="og:description"
		content="Managed adaptive video, AI shorts, live streaming, and signed playback — get early access."
	/>
</svelte:head>

<section class="mx-auto max-w-lg px-6 pt-32 pb-24">
	{#if done}
		<div class="text-center">
			<span
				class="mx-auto mb-5 flex h-14 w-14 items-center justify-center rounded-full bg-success/10 text-success"
			>
				<Icon icon={CheckCircle} size={30} />
			</span>
			<h1 class="text-2xl font-semibold tracking-tight">You're on the list 🎉</h1>
			<p class="mt-3 text-muted">
				Thanks, {name.split(' ')[0]}. We'll reach out on your email
				{#if whatsapp}(or WhatsApp){/if} as early access opens.
			</p>
			<a href="/" class="mt-8 inline-block text-sm text-accent hover:underline">← Back to home</a>
		</div>
	{:else}
		<div class="mb-8">
			<h1 class="text-3xl font-semibold tracking-tight">Get early access</h1>
			<p class="mt-2 text-muted">
				Ferrite Stream Cloud is launching soon — join the waitlist and we'll invite you in. A few quick
				questions help us build the right thing.
			</p>
		</div>

		<form
			onsubmit={(e) => {
				e.preventDefault();
				submit();
			}}
			class="flex flex-col gap-4"
		>
			<div class="grid gap-4 sm:grid-cols-2">
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-medium text-muted">Name *</span>
					<input bind:value={name} placeholder="Your name" autocomplete="name" class={inputCls} />
				</label>
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-medium text-muted">Email *</span>
					<input
						bind:value={email}
						type="email"
						placeholder="you@company.com"
						autocomplete="email"
						class={inputCls}
					/>
				</label>
			</div>

			<div class="grid gap-4 sm:grid-cols-2">
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-medium text-muted">WhatsApp <span class="text-muted/60">(optional)</span></span>
					<input bind:value={whatsapp} placeholder="+880 1XXX-XXXXXX" class={inputCls} />
				</label>
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-medium text-muted">Country</span>
					<input bind:value={country} class={inputCls} />
				</label>
			</div>

			<label class="flex flex-col gap-1.5">
				<span class="text-xs font-medium text-muted">What are you building?</span>
				<textarea
					bind:value={useCase}
					rows="2"
					placeholder="e.g. an e-learning platform, a creator app, internal training videos…"
					class={inputCls}
				></textarea>
			</label>

			<div class="grid gap-4 sm:grid-cols-2">
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-medium text-muted">Expected volume</span>
					<select bind:value={volume} class={inputCls}>
						<option value="" disabled selected>Select…</option>
						{#each volumes as v (v)}<option value={v}>{v}</option>{/each}
					</select>
				</label>
				<label class="flex flex-col gap-1.5">
					<span class="text-xs font-medium text-muted">Plan of interest</span>
					<select bind:value={plan} class={inputCls}>
						<option value="" disabled selected>Select…</option>
						{#each plans as p (p)}<option value={p}>{p}</option>{/each}
					</select>
				</label>
			</div>

			<label class="flex flex-col gap-1.5">
				<span class="text-xs font-medium text-muted">
					Preferred way to pay <span class="text-muted/60">(helps us support local options)</span>
				</span>
				<select bind:value={payment} class={inputCls}>
					<option value="" disabled selected>Select…</option>
					{#each payments as p (p)}<option value={p}>{p}</option>{/each}
				</select>
			</label>

			{#if error}<p class="text-sm text-danger">{error}</p>{/if}

			<button
				type="submit"
				disabled={busy}
				class="mt-2 inline-flex items-center justify-center gap-2 rounded-lg bg-accent px-5 py-3 text-sm font-medium text-accent-fg transition-opacity hover:opacity-90 disabled:opacity-50"
			>
				{busy ? 'Joining…' : 'Join the waitlist'}
				<Icon icon={ArrowRight} size={16} />
			</button>
			<p class="text-center text-xs text-muted">
				No spam. We'll only email you about early access.
			</p>
		</form>
	{/if}
</section>
