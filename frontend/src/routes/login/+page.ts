import { redirect } from "@sveltejs/kit";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async () => {
  if (localStorage.getItem("jwt")) {
    throw redirect(300, "/");
  }
};
