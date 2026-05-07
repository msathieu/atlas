import { redirect } from "@sveltejs/kit";
import { API_HOST, bufferToBase64, signRequest } from "./util.ts";

export async function authenticatedFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
  fn?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>,
  contentType?: string,
): Promise<Response> {
  if (!init) {
    init = {};
  }
  if (!contentType) {
    contentType = "application/json";
  }
  if (!fn) {
    fn = fetch;
  }
  const jwt = localStorage.getItem("jwt");
  if (!jwt) {
    throw redirect(300, "/login");
  }
  const url = API_HOST + input;
  init["headers"] = new Headers({
    "Authorization": `Bearer ${jwt}`,
    "Content-Type": contentType,
  });
  const encoder = new TextEncoder();
  let body = init["body"];
  if (!body) {
    body = "";
  }
  if (typeof body === "string") {
    body = encoder.encode(body as string);
  }
  const url_hash = await crypto.subtle.digest("SHA-512", encoder.encode(input.toString()));
  const body_hash = await crypto.subtle.digest("SHA-512", body as ArrayBuffer);
  let method = init["method"];
  if (!method) {
    method = "GET";
  }
  const timestamp = Math.floor(Date.now() / 1000);
  init["headers"].append("X-Request-Timestamp", String(timestamp));
  init["headers"].append("X-Request-Signature", await signRequest(method + "-" + String(timestamp) + "-" + bufferToBase64(url_hash) + "-" + bufferToBase64(body_hash)));
  const response = await fn(url, init);
  if (response.status === 401) {
    localStorage.clear();
    location.reload();
    throw null;
  }
  return response;
}
