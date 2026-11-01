// Takes the screenshots of docs/screenshots/ from a server filled by seed.mjs.
//
//   BASE_URL   the running server (default http://127.0.0.1:3000)
//   OUT_DIR    where the PNGs go (default ../../docs/screenshots)
//   ONLY       comma-separated shot names (see SHOTS), to take just these

import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { PASSWORD } from './data.mjs';

const base = process.env.BASE_URL ?? 'http://127.0.0.1:3000';
const out =
	process.env.OUT_DIR ?? join(dirname(fileURLToPath(import.meta.url)), '../../docs/screenshots');
const only = process.env.ONLY?.split(',');
mkdirSync(out, { recursive: true });

const DESKTOP = { width: 1280, height: 900 };
const PHONE = { width: 390, height: 844 };

const browser = await chromium.launch();

/** A logged-in page with fixed time zone, language and theme. */
async function open({
	viewport = DESKTOP,
	theme = 'light',
	locale = 'en-US',
	user = 'fiona'
} = {}) {
	const context = await browser.newContext({
		viewport,
		deviceScaleFactor: viewport === PHONE ? 2 : 1,
		locale,
		timezoneId: 'Europe/Berlin',
		colorScheme: theme,
		reducedMotion: 'reduce'
	});
	await context.addInitScript((theme) => {
		try {
			localStorage.setItem('theme', theme);
		} catch {}
	}, theme);
	if (user) {
		const response = await context.request.post(`${base}/api/session`, {
			data: { login: user, password: PASSWORD }
		});
		if (!response.ok()) throw new Error(`login ${user}: ${response.status()}`);
	}
	return context.newPage();
}

/** Opens a path and waits until the page has drawn its data and covers. */
async function visit(page, path) {
	await page.goto(base + path, { waitUntil: 'load' });
	// The pages load their data after the first paint. The live updates keep a
	// connection open, so the network never goes quite idle.
	await page.waitForLoadState('networkidle', { timeout: 5000 }).catch(() => {});
	await page.evaluate(() =>
		Promise.all(
			[...document.images]
				.filter((img) => !img.complete)
				.map(
					(img) =>
						new Promise((done) => {
							img.addEventListener('load', done, { once: true });
							img.addEventListener('error', done, { once: true });
							setTimeout(done, 5000);
						})
				)
		)
	);
	// Charts grow in.
	await page.waitForTimeout(800);
}

/** A section of the page by its heading. */
const section = (page, heading) =>
	page
		.locator('section')
		.filter({ has: page.locator('h2', { hasText: heading }) })
		.first();

/**
 * A full-width strip of the page: from the top of `from` (or the page top)
 * down to the top of `until` (or the page end), or `height` pixels. `around`
 * takes just one element with some room around it.
 */
async function shot(page, name, { from, until, height, around } = {}) {
	const top = async (locator) => {
		const box = await locator.boundingBox();
		if (!box) throw new Error(`${name}: element not found`);
		return box.y + (await page.evaluate(() => window.scrollY));
	};
	const width = page.viewportSize().width;
	const full = await page.evaluate(() => document.documentElement.scrollHeight);
	let clip;
	if (around) {
		const box = await around.boundingBox();
		const y = await top(around);
		const pad = 24;
		clip = {
			x: Math.max(0, box.x - pad),
			y: Math.max(0, y - pad),
			width: Math.min(width, box.width + 2 * pad),
			height: box.height + 2 * pad
		};
	} else {
		const y = from ? Math.max(0, (await top(from)) - 24) : 0;
		const end = until ? (await top(until)) - 16 : height ? y + height : full;
		clip = { x: 0, y, width, height: Math.min(full, end) - y };
	}
	await page.screenshot({ path: join(out, `${name}.png`), fullPage: true, clip });
	console.log(name);
}

/** The href of the first link with this text. */
const href = (page, text) =>
	page.getByRole('link', { name: text, exact: true }).first().getAttribute('href');

const year = new Date().getFullYear();

// Last year's review is a whole year whenever this runs.
const lastYear = new Date().getFullYear() - 1;

/** The address of an artist or album page, from its link on the profile page. */
const linked = (name) => async (page) => {
	await visit(page, '/u/fiona');
	return href(page, name);
};

// One file each, docs/screenshots/<name>.png. `path` is where to go (or finds
// it), `clip` what to take (see shot()), `as` how to open the page (see open()).
const SHOTS = [
	{
		name: 'profile',
		path: '/u/fiona',
		clip: (page) => ({ until: section(page, 'Listening clock') })
	},
	{
		name: 'charts',
		path: '/u/fiona?days=30',
		clip: (page) => ({
			from: page.locator('main nav').first(),
			until: section(page, 'Listening clock')
		})
	},
	{
		name: 'listening-clock',
		path: '/u/fiona',
		clip: (page) => ({ around: section(page, 'Listening clock') })
	},
	{
		name: 'lost-and-found',
		path: '/u/fiona',
		clip: (page) => ({ around: section(page, 'Lost & found') })
	},
	{
		name: 'artists-over-the-years',
		path: '/u/fiona',
		clip: (page) => ({ around: section(page, 'Top artists over the years') })
	},
	{ name: 'artist', path: linked('Die Ärzte'), clip: () => ({ height: 820 }) },
	{ name: 'album', path: linked('Geräusch'), clip: () => ({ height: 820 }) },
	{
		name: 'year-in-review',
		path: `/u/fiona/default/year/${lastYear}`,
		clip: () => ({ height: 1800 })
	},
	{ name: 'compare', path: '/u/fiona/default/compare/alex', clip: () => ({ height: 1500 }) },
	{ name: 'search', path: '/u/fiona/default/search?q=ra', clip: () => ({ height: 620 }) },
	{ name: 'dark', path: '/u/fiona', as: { theme: 'dark' }, clip: () => ({ height: 1400 }) },
	{
		name: 'phone',
		path: '/u/fiona',
		as: { viewport: PHONE },
		clip: () => ({ height: PHONE.height })
	},
	{
		name: 'german',
		path: `/u/fiona/default/year/${lastYear}`,
		as: { locale: 'de-DE' },
		clip: () => ({ height: 1200 })
	},
	{ name: 'settings', path: '/settings', clip: () => ({ height: 1100 }) },
	{ name: 'history', path: '/settings/history/default', clip: () => ({ height: 1100 }) },
	{ name: 'merges', path: '/merges', clip: () => ({ height: 720 }) }
];

for (const { name, path, as, clip } of SHOTS) {
	if (only && !only.includes(name)) continue;
	const page = await open(as);
	await visit(page, typeof path === 'string' ? path : await path(page));
	await shot(page, name, clip(page));
	await page.context().close();
}
await browser.close();
