import { authenticatedFetch } from "./auth.ts";

export async function updateContainer(id: string, memory_limit: number, storage_size: number) {
  const data: { memory_limit?: number; storage_size?: number } = {};
  if (memory_limit > 0) {
    data["memory_limit"] = memory_limit;
  }
  if (storage_size > 0) {
    data["storage_size"] = storage_size;
  }
  await authenticatedFetch(`/api/containers/${id}`, {
    method: "PATCH",
    body: JSON.stringify(data),
  });
}
