// Fills an empty musicbanana with the made-up histories of data.mjs: two
// accounts through the command line, their listens through the scrobble API,
// and album covers straight into the cover cache, so nothing is fetched from
// the Cover Art Archive.
//
//   MUSICBANANA   how to run the command line (default `musicbanana`), e.g.
//                 `docker exec -i musicbanana musicbanana`
//   BASE_URL      the running server (default http://127.0.0.1:3000)
//   COVER_CACHE   the server's COVER_ART_CACHE directory; no covers when unset

import { execSync } from 'node:child_process';
import { chmodSync, mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { chromium } from 'playwright';
import { COVERS, PASSWORD, USERS, history } from './data.mjs';

const cli = process.env.MUSICBANANA ?? 'musicbanana';
const base = process.env.BASE_URL ?? 'http://127.0.0.1:3000';

function run(args, input) {
	return execSync(`${cli} ${args}`, {
		input,
		encoding: 'utf8',
		stdio: ['pipe', 'pipe', 'inherit']
	}).trim();
}

/** One request to the scrobble API, as a player sends it. */
async function scrobble(token, body) {
	const response = await fetch(`${base}/api/listenbrainz/1/submit-listens`, {
		method: 'POST',
		headers: { Authorization: `Token ${token}`, 'Content-Type': 'application/json' },
		body: JSON.stringify(body)
	});
	if (!response.ok) throw new Error(`submit-listens: ${response.status} ${await response.text()}`);
}

async function submit(token, listens) {
	for (let i = 0; i < listens.length; i += 1000) {
		await scrobble(token, { listen_type: 'import', payload: listens.slice(i, i + 1000) });
	}
}

// For the profile header. Without a length it lasts ten minutes, enough for capture.mjs.
const nowPlaying = (token) =>
	scrobble(token, {
		listen_type: 'playing_now',
		payload: [
			{
				track_metadata: {
					artist_name: 'Die Ärzte',
					track_name: 'Unrockbar',
					release_name: 'Geräusch',
					additional_info: { submission_client: 'Navidrome', media_player: 'Supersonic' }
				}
			}
		]
	});

async function covers(dir) {
	const caa = join(dir, 'caa');
	mkdirSync(caa, { recursive: true });
	// The server runs as another user in the image and touches the files it serves.
	chmodSync(caa, 0o777);
	const browser = await chromium.launch();
	const page = await browser.newPage({ viewport: { width: 500, height: 500 } });
	for (const cover of COVERS) {
		await page.setContent(coverHtml(cover));
		for (const size of [500, 250]) {
			await page.setViewportSize({ width: size, height: size });
			const file = join(caa, `${cover.mbid}-${size}`);
			writeFileSync(file, await page.screenshot({ type: 'jpeg', quality: 85 }));
			chmodSync(file, 0o666);
		}
	}
	await browser.close();
}

// A plain made-up sleeve: the album's colour, a circle, artist and title.
function coverHtml({ artist, title, color }) {
	const esc = (s) => s.replace(/[&<>]/g, (c) => `&#${c.charCodeAt(0)};`);
	return `<!doctype html><meta charset="utf-8"><style>
		html,body{margin:0;height:100%}
		body{background:linear-gradient(135deg,${color},#111 160%);color:#fff;font:600 7vw/1.15 system-ui,sans-serif;
			display:flex;flex-direction:column;justify-content:flex-end;padding:8vw;box-sizing:border-box;position:relative;overflow:hidden}
		.c{position:absolute;right:-18vw;top:-18vw;width:70vw;height:70vw;border-radius:50%;border:5vw solid rgba(255,255,255,.18)}
		.a{font-weight:400;opacity:.8;font-size:5vw;margin-top:2vw}
	</style><div class="c"></div><div>${esc(title)}</div><div class="a">${esc(artist)}</div>`;
}

const now = Date.now();
for (const [name, user] of Object.entries(USERS)) {
	run(`account create ${name} --email ${user.email}`, `${PASSWORD}\n`);
	if (user.admin) run(`account admin ${name}`);
	const token = run(`token create --user ${name} --label Navidrome`);
	const listens = history(name, { now, ...user.history });
	await submit(token, listens);
	console.log(`${name}: ${listens.length} listens`);
	if (name === 'fiona') await nowPlaying(token);
}
if (process.env.COVER_CACHE) {
	await covers(process.env.COVER_CACHE);
	console.log(`${COVERS.length} covers in ${process.env.COVER_CACHE}`);
}
