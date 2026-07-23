import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-node';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	// Pin the dev port. strictPort makes Vite fail loudly if 5173 is taken
	// instead of silently hopping to a random port (keeps dev URLs consistent).
	server: { port: 5173, strictPort: true },
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) => filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter()
		})
	],
	// phosphor-svelte ships uncompiled .svelte; let Vite compile it for SSR.
	ssr: { noExternal: ['phosphor-svelte'] }
});
