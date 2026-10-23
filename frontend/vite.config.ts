/// <reference types="vitest/config" />
import tailwindcss from '@tailwindcss/vite';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { svelteTesting } from '@testing-library/svelte/vite';
import { defineConfig } from 'vite';

if (process.env.VITEST) {
	// The same time zone and language on every machine, so dates and numbers in
	// tests come out the same. Set here, so the test workers start with them.
	process.env.TZ = 'Europe/Berlin';
	process.env.LC_ALL = 'en_US.UTF-8';
}

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
		}),
		// Only under Vitest: Svelte's browser build and a clean DOM after each test.
		svelteTesting()
	],
	server: {
		// During `pnpm dev`, API calls go to the backend on its default port.
		proxy: { '/api': 'http://127.0.0.1:3000' }
	},
	test: {
		environment: 'happy-dom',
		include: ['src/**/*.test.ts'],
		setupFiles: ['src/test/setup.ts'],
		coverage: {
			provider: 'v8',
			include: ['src/**/*.{ts,svelte}'],
			exclude: ['src/**/*.test.ts', 'src/test/**', 'src/app.d.ts'],
			// Paths in lcov.info from the repository root, as Codecov sees them.
			reporter: ['text-summary', ['lcov', { projectRoot: '..' }]]
		}
	}
});
