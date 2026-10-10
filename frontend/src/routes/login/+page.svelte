<script lang="ts">
import { Passkeys } from "@capawesome/capacitor-passkeys";
import { toast } from "svelte-sonner";
import { verifyResponseSignature } from "#lib/auth.ts";
import { API_HOST, bufferToBase64, encryptToServer, getDatabase } from "#lib/util.ts";
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
	const encryptedBody = await encryptToServer(body);
	const response = await fetch(`${API_HOST}/api/login`, {
		method: "POST",
		body: encryptedBody,
		headers: { "Content-Type": "application/octet-stream" },
	});
	if (!response.ok) {
		toast.error("Invalid Credentials");
		return;
	}
	const bodyHash = await crypto.subtle.digest("SHA-512", encryptedBody);
	await verifyResponseSignature(response, `login-${bufferToBase64(bodyHash)}`);
	const json = await response.json();
	await finishAuth(username, json.jwt, keyPair);
}
async function passkeyLogin() {
	const startBody = JSON.stringify({ username });
	const startRes = await fetch(`${API_HOST}/api/passkeys/start_authentication`, { method: "POST", body: startBody });
	if (!startRes.ok) {
		toast.error("Invalid Credentials");
		return;
	}
	const encoder = new TextEncoder();
	const startHash = await crypto.subtle.digest("SHA-512", encoder.encode(startBody));
	await verifyResponseSignature(startRes, `passkey_start-${bufferToBase64(startHash)}`);
	const startData = await startRes.json();
	const credential = await Passkeys.getPasskey(startData.challenge.publicKey);
	const keyPair = await crypto.subtle.generateKey("Ed25519", false, ["sign", "verify"]);
	const pubkey = bufferToBase64(await crypto.subtle.exportKey("raw", keyPair.publicKey));
	const encryptedBody = await encryptToServer(JSON.stringify({ flow_id: startData.flow_id, credential, pubkey }));
	const finishRes = await fetch(`${API_HOST}/api/passkeys/finish_authentication`, {
		method: "POST",
		body: encryptedBody,
		headers: { "Content-Type": "application/octet-stream" },
	});
	if (!finishRes.ok) {
		toast.error("Invalid Credentials");
		return;
	}
	const finishHash = await crypto.subtle.digest("SHA-512", encryptedBody);
	await verifyResponseSignature(finishRes, `passkey_finish-${bufferToBase64(finishHash)}`);
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
				<button type="submit" class="btn btn-primary">Login</button>
				<div class="divider"></div>
				<button type="button" class="btn btn-primary" onclick={async () => await runLogin(passkeyLogin)}>Login With Passkey</button>
			</fieldset>
		</form>
	</div>
</div>
<footer class="footer footer-center">
	<a href="/THIRD-PARTY.txt" class="link link-primary" target="_blank">Third-Party Licenses</a>
</footer>
