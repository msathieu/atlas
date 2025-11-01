import { redirect } from "@sveltejs/kit";
import { API_HOST } from "./util.ts";

export async function authenticatedFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
  fn?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>,
): Promise<Response> {
  if (!init) {
    init = {};
  }
  const jwt = localStorage.getItem("jwt");
  if (!jwt) {
    throw redirect(300, "/login");
  }
  input = API_HOST + input;
  init["headers"] = new Headers({
    "Authorization": `Bearer ${jwt}`,
    "Content-Type": "application/json",
  });
  let response;
  if (fn) {
    response = await fn(input, init);
  } else {
    response = await fetch(input, init);
  }
  if (response.status === 401) {
    localStorage.removeItem("jwt");
    throw redirect(300, "/login");
  }
  return response;
}
