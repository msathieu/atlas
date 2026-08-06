import type { PageLoad } from "./$types.ts";

export const load: PageLoad = ({ url }) => {
  const urlParam = url.searchParams.get("url");
  if (!urlParam) {
    globalThis.close();
    throw new Error("Missing url parameter");
  }
  const urlObject = new URL(urlParam);
  const repo = urlObject.searchParams.get("repo");
  if (urlObject.protocol !== "web+atlasrepo:" || urlObject.host !== "open" || !repo) {
    globalThis.close();
    throw new Error("Invalid url parameter");
  }
  return { repo };
};
