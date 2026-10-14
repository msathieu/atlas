import { type GetPasskeyResult, Passkeys } from "@capawesome/capacitor-passkeys";
import { API_HOST, bufferToBase64, signRequest, verifySignature } from "./util.ts";

export type PasskeyVerification = { flow_id: string; credential: GetPasskeyResult };

export async function getPasskeyVerification(response: Response): Promise<PasskeyVerification> {
	const { challenge, flow_id } = await response.json();
	const credential = await Passkeys.getPasskey(challenge.publicKey);
	return { flow_id, credential };
}
export async function verifyResponseSignature(response: Response, requestSignature: string): Promise<void> {
	const encoder = new TextEncoder();
	const responseHash = await crypto.subtle.digest("SHA-512", encoder.encode(await response.clone().text()));
	const responseSignaturePlaintext = `${requestSignature}-${bufferToBase64(responseHash)}`;
	if (!(await verifySignature(responseSignaturePlaintext, response.headers.get("X-Response-Signature") || ""))) {
		location.reload();
		throw new Error("Invalid response signature");
	}
}
export async function authenticatedFetch(input: RequestInfo | URL, init?: RequestInit, fn?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>): Promise<Response> {
	if (!init) {
		init = {};
	}
	if (!fn) {
		fn = fetch;
	}
	const jwt = localStorage.getItem("jwt");
	if (!jwt) {
		throw new Error("Unauthorized");
	}
	const url = API_HOST + input;
	init.headers = new Headers(init.headers);
	init.headers.set("Authorization", `Bearer ${jwt}`);
	if (!init.headers.has("Content-Type")) {
		init.headers.set("Content-Type", "application/json");
	}
	const encoder = new TextEncoder();
	let body = init.body;
	if (!body) {
		body = "";
	}
	if (typeof body === "string") {
		body = encoder.encode(body as string);
	}
	const urlHash = await crypto.subtle.digest("SHA-512", encoder.encode(input.toString()));
	const bodyHash = await crypto.subtle.digest("SHA-512", body as ArrayBuffer);
	let method = init.method;
	if (!method) {
		method = "GET";
	}
	const timestamp = Math.floor(Date.now() / 1000);
	const nonce = crypto.randomUUID();
	init.headers.append("X-Request-Nonce", nonce);
	init.headers.append("X-Request-Timestamp", String(timestamp));
	const requestSignature = await signRequest(`${method}-${String(timestamp)}-${nonce}-${bufferToBase64(urlHash)}-${bufferToBase64(bodyHash)}`);
	init.headers.append("X-Request-Signature", requestSignature);
	const response = await fn(url, init);
	if (response.status === 401) {
		localStorage.removeItem("jwt");
		location.reload();
		throw new Error("Unauthorized");
	}
	if (response.status !== 200 && response.status !== 204) {
		throw new Error("Bad Request");
	}
	await verifyResponseSignature(response, requestSignature);
	return response;
}
export async function refreshJWT() {
	const response = await authenticatedFetch(`/api/refresh_jwt`, {
		method: "POST",
	});
	const data = await response.json();
	if (localStorage.getItem("jwt")) {
		localStorage.setItem("jwt", data.jwt);
	}
}
