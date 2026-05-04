import { redirect } from "@sveltejs/kit";
import { API_HOST } from "./util.ts";

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
  input = API_HOST + input;
  init["headers"] = new Headers({
    "Authorization": `Bearer ${jwt}`,
    "Content-Type": contentType,
  });
  const response = await fetch(input, init);
  if (response.status === 401) {
    localStorage.clear();
    throw redirect(300, "/login");
  }
  return response;
}
