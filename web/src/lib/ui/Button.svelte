<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';

	type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
	type Size = 'sm' | 'md';

	interface Props extends HTMLButtonAttributes {
		variant?: Variant;
		size?: Size;
		children: Snippet;
	}

	let { variant = 'primary', size = 'md', class: klass = '', children, ...rest }: Props = $props();

	const base =
		'inline-flex items-center justify-center gap-2 rounded-lg font-medium transition-[background-color,opacity,transform] duration-150 active:scale-[0.97] focus-visible:outline-2 disabled:opacity-50 disabled:pointer-events-none';

	const variants: Record<Variant, string> = {
		primary: 'bg-accent text-accent-fg hover:opacity-90',
		secondary: 'bg-surface-2 text-fg border border-border hover:bg-surface',
		ghost: 'text-muted hover:text-fg hover:bg-surface-2',
		danger: 'bg-danger text-white hover:opacity-90'
	};

	const sizes: Record<Size, string> = {
		sm: 'h-8 px-3 text-sm',
		md: 'h-10 px-4 text-sm'
	};
</script>

<button class={`${base} ${variants[variant]} ${sizes[size]} ${klass}`} {...rest}>
	{@render children()}
</button>
