import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// SPA build: the Rust backend serves build/ and falls back to index.html.
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	server: {
		// During `npm run dev`, API calls go to the backend on its default port.
		proxy: { '/api': 'http://127.0.0.1:3000' }
	}
});
