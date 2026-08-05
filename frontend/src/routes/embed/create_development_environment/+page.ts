import type { PageLoad } from "./$types.ts";

export const load: PageLoad = ({ url }) => {
  const urlParam = url.searchParams.get("url");
  const start = "web+atlasrepo://open?repo=";
  if (!urlParam || !urlParam.startsWith(start)) {
    globalThis.close();
  }
  const repo = urlParam?.slice(start.length);
  return { repo };
};
