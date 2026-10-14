<script lang="ts">
import { toast } from "svelte-sonner";
import { getPasskeyVerification } from "#lib/auth.ts";
import { API_HOST, bufferToBase64, encryptedFetch, getDatabase } from "#lib/util.ts";
import { goto } from "$app/navigation";
import { page } from "$app/state";

let username = $state(localStorage.getItem("username") || "");
let password = $state("");
let loginPending = $state(false);
let redirect = page.url.searchParams.get("redirect");

async function runLogin(authenticate: () => Promise<void>) {
	if (loginPending) {
		return;
	}
	loginPending = true;
	try {
		await authenticate();
	} catch {
		toast.error("Login Failed");
	} finally {
		loginPending = false;
	}
}
async function login() {
	const keyPair = await crypto.subtle.generateKey("Ed25519", false, ["sign", "verify"]);
	const pubkey = bufferToBase64(await crypto.subtle.exportKey("raw", keyPair.publicKey));
	const body = JSON.stringify({
		username,
		password,
		pubkey,
		timestamp: Math.floor(Date.now() / 1000),
	});
	const response = await encryptedFetch(`${API_HOST}/api/login`, { method: "POST", body }, fetch, "login");
	if (!response.ok) {
		toast.error("Invalid Credentials");
		return;
	}
	const json = await response.json();
	await finishAuth(username, json.jwt, keyPair);
}
async function passkeyLogin() {
	const startBody = JSON.stringify({ username });
	const startRes = await encryptedFetch(`${API_HOST}/api/passkeys/start_authentication`, { method: "POST", body: startBody }, fetch, "passkey_start");
	if (!startRes.ok) {
		toast.error("Invalid Credentials");
		return;
	}
	const verification = await getPasskeyVerification(startRes);
	const keyPair = await crypto.subtle.generateKey("Ed25519", false, ["sign", "verify"]);
	const pubkey = bufferToBase64(await crypto.subtle.exportKey("raw", keyPair.publicKey));
	const finishRes = await encryptedFetch(`${API_HOST}/api/passkeys/finish_authentication`, { method: "POST", body: JSON.stringify({ ...verification, pubkey }) }, fetch, "passkey_finish");
	if (!finishRes.ok) {
		toast.error("Invalid Credentials");
		return;
	}
	const json = await finishRes.json();
	await finishAuth(username, json.jwt, keyPair);
}
async function finishAuth(username: string, jwt: string, keyPair: CryptoKeyPair) {
	password = "";
	const db = getDatabase();
	await db.table("keys").put({ name: "jwt_signing", key: keyPair.privateKey });
	localStorage.setItem("username", username);
	localStorage.setItem("jwt", jwt);
	if (!redirect) {
		await goto("/");
		return;
	}
	let target = new URL(redirect, page.url.origin);
	await goto(target.pathname + target.search);
}
</script>

<div class="flex" style="height: 90%">
	<div class="m-auto">
		<h1 class="text-6xl"><img src="/logo.png" alt="logo" class="inline h-32">Atlas</h1>
		<br>
		<form
			onsubmit={async (e) => {
				e.preventDefault();
				await runLogin(login);
			}}
		>
			<fieldset class="fieldset" disabled={loginPending}>
				<div>
					<label for="username">Username</label>
					<input id="username" type="text" placeholder="Username" class="input" required autocomplete="username" bind:value={username}>
				</div>
				<div>
					<label for="password">Password</label>
					<input id="password" type="password" placeholder="Password" class="input" autocomplete="current-password" bind:value={password}>
				</div>
				<button type="submit" class="btn btn-primary">Login With Password</button>
				<div class="divider">or</div>
				<button type="button" class="btn btn-primary" onclick={async () => await runLogin(passkeyLogin)}>Login With Passkey</button>
			</fieldset>
		</form>
	</div>
</div>
<footer class="footer footer-center">
	<a href="/THIRD-PARTY.txt" class="link link-primary" target="_blank">Third-Party Licenses</a>
</footer>
