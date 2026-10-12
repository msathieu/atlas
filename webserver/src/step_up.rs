use crate::{
    auth::authenticate,
    get_db,
    passkey::{StartPasskeyAuthenticationResponse, get_webauthn, load_authentication_passkeys, verify_passkey_authentication},
    util::{sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::OriginalUri,
    http::{HeaderMap, StatusCode},
};
use redis::AsyncTypedCommands;
use serde::{Deserialize, Serialize};
use shared_server::{get_valkey_client, models::User, verify_password};
use uuid::Uuid;
use webauthn_rs::prelude::{PasskeyAuthentication, PublicKeyCredential};

#[derive(Deserialize)]
pub struct PasskeyVerification {
    flow_id: Uuid,
    credential: PublicKeyCredential,
}
#[derive(Deserialize)]
pub struct StepUpVerification {
    password: Option<String>,
    passkey: Option<PasskeyVerification>,
}
#[derive(Deserialize, Serialize)]
struct StepUpAuthentication {
    authentication: PasskeyAuthentication,
    pubkey: String,
}

pub async fn start_passkey_step_up(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<StartPasskeyAuthenticationResponse>), StatusCode> {
    let (user, pubkey, _) = authenticate(&headers).await?;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let pubkey = pubkey.ok_or(StatusCode::UNAUTHORIZED)?;
    let uid = user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut db = get_db().await?;
    let credentials = load_authentication_passkeys(&mut db, uid).await?;
    if credentials.is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }
    let (challenge, authentication) = get_webauthn(&mut db).await?.start_passkey_authentication(&credentials).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let flow_id = Uuid::new_v4();
    let state = serde_json::to_string(&StepUpAuthentication { authentication, pubkey }).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut valkey = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    valkey.set_ex(format!("PASSKEY_STEP_UP:{flow_id}"), state, 300).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = StartPasskeyAuthenticationResponse { challenge, flow_id };
    Ok((sign_response(signature, Some(&response))?, Json(response)))
}
pub async fn verify_step_up(headers: &HeaderMap, user: &User, verification: StepUpVerification) -> Result<(), StatusCode> {
    match (verification.password, verification.passkey) {
        (Some(password), None) => {
            if user.password_enabled {
                verify_password(user.password.clone(), password).await.or(Err(StatusCode::FORBIDDEN))
            } else {
                Err(StatusCode::FORBIDDEN)
            }
        }
        (None, Some(passkey)) => {
            let uid = user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
            let pubkey = authenticate(headers).await?.1.ok_or(StatusCode::UNAUTHORIZED)?;
            let mut valkey = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
            let state: Option<String> = valkey.get_del(format!("PASSKEY_STEP_UP:{}", passkey.flow_id)).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
            let state: StepUpAuthentication = serde_json::from_str(&state.ok_or(StatusCode::FORBIDDEN)?).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
            if state.pubkey != pubkey {
                return Err(StatusCode::FORBIDDEN);
            }
            let mut db = get_db().await?;
            verify_passkey_authentication(&mut db, uid, &passkey.credential, &state.authentication, StatusCode::FORBIDDEN).await
        }
        _ => Err(StatusCode::FORBIDDEN),
    }
}
