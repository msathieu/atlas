import type { PasskeyVerification } from "./auth.ts";
import { encryptedFetch } from "./util.ts";

export type StepUpVerification = { password?: string; passkey?: PasskeyVerification };
export type StepUpAction = (verification: StepUpVerification) => Promise<void>;

export async function stepUpFetch(input: RequestInfo | URL, method: string, body: any, verification: StepUpVerification): Promise<Response> {
	body.verification = verification;
	return encryptedFetch(input, { method, body: JSON.stringify(body) });
}
