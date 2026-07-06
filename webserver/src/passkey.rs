use crate::{
    auth::{JWT_EXPIRY, authenticate, generate_jwt},
    get_db,
    util::{decrypt_body, sign_response, validate_signature},
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Json,
    body::Bytes,
    extract::OriginalUri,
    http::{HeaderMap, StatusCode},
};
use base64::{Engine, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use shared_server::{
    get_valkey_client,
    models::{Passkey, Setting, User},
    schema::{passkeys, settings, users},
};
use uuid::Uuid;
use webauthn_rs::{
    Webauthn, WebauthnBuilder,
    prelude::{CreationChallengeResponse, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential, RequestChallengeResponse, Url},
};

#[derive(Deserialize)]
pub struct StartPasskeyRegistrationRequest {
    password: String,
}
#[derive(Deserialize)]
pub struct StartPasskeyAuthenticationRequest {
    username: String,
}
#[derive(Serialize)]
pub struct StartPasskeyAuthenticationResponse {
    challenge: RequestChallengeResponse,
    flow_id: Uuid,
}
#[derive(Deserialize)]
pub struct FinishPasskeyAuthenticationRequest {
    pubkey: String,
    credential: PublicKeyCredential,
    flow_id: Uuid,
}
#[derive(Serialize)]
pub struct FinishPasskeyAuthenticationResponse {
    jwt: String,
}
#[derive(Deserialize, Serialize)]
struct PasskeyAuthenticationValkey {
    authentication: PasskeyAuthentication,
    uid: Uuid,
}

pub fn get_webauthn() -> Result<Webauthn, StatusCode> {
    let mut db = get_db()?;
    let web_host: Setting = settings::table.find("webserver_host").get_result(&mut db).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let url = Url::parse(&format!("https://{}", web_host.value)).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let webauthn = WebauthnBuilder::new(&web_host.value, &url)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
        .build()
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok(webauthn)
}
pub async fn start_passkey_registration(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreationChallengeResponse>), StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: StartPasskeyRegistrationRequest = serde_json::from_str(&decrypt_body(body.clone())?).or(Err(StatusCode::BAD_REQUEST))?;
    let hash = PasswordHash::new(&user.password).or(Err(StatusCode::FORBIDDEN))?;
    Argon2::default().verify_password(request.password.as_bytes(), &hash).or(Err(StatusCode::FORBIDDEN))?;
    let webauthn = get_webauthn()?;
    let (challenge, registration) = webauthn
        .start_passkey_registration(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?, &user.username, &user.username, None)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let key = format!("PASSKEY_REGISTRATION:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?);
    let registration_json = serde_json::to_string(&registration).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let _: Option<String> = valkey_client.set_ex(key, registration_json, 300).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok((sign_response(signature, Some(&challenge))?, Json(challenge)))
}
pub async fn finish_passkey_registration(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers)?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: RegisterPublicKeyCredential = serde_json::from_str(&String::from_utf8(body.to_vec()).or(Err(StatusCode::BAD_REQUEST))?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let key = format!("PASSKEY_REGISTRATION:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?);
    let registration_json: Option<String> = valkey_client.get_del(key).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let registration: PasskeyRegistration = match registration_json {
        Some(json) => serde_json::from_str(&json).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
        None => return Err(StatusCode::BAD_REQUEST),
    };
    let webauthn = get_webauthn()?;
    let passkey = webauthn.finish_passkey_registration(&request, &registration).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let passkey_json = serde_json::to_string(&passkey).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let passkey_db = Passkey {
        id: passkey.cred_id().to_vec(),
        uid: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        passkey: passkey_json,
    };
    diesel::insert_into(passkeys::table).values(passkey_db).execute(&mut db).or(Err(StatusCode::BAD_REQUEST))?;
    sign_response::<String>(signature, None)
}
pub async fn start_passkey_authentication(body: Bytes) -> Result<(HeaderMap, Json<StartPasskeyAuthenticationResponse>), StatusCode> {
    let webauthn = get_webauthn()?;
    let request: StartPasskeyAuthenticationRequest = serde_json::from_str(&String::from_utf8(body.to_vec()).or(Err(StatusCode::BAD_REQUEST))?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db()?;
    let user: User = users::table.filter(users::username.eq(request.username)).first(&mut db).or(Err(StatusCode::UNAUTHORIZED))?;
    let passkeys_db: Vec<Passkey> = passkeys::table
        .filter(passkeys::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .load(&mut db)
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut passkeys = vec![];
    for i in passkeys_db {
        let passkey: webauthn_rs::prelude::Passkey = serde_json::from_str(&i.passkey).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        passkeys.push(passkey);
    }
    let (challenge, authentication) = webauthn.start_passkey_authentication(&passkeys).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let flow_id = Uuid::new_v4();
    let key = format!("PASSKEY_AUTHENTICATION:{}", flow_id);
    let authentication_data = PasskeyAuthenticationValkey {
        authentication,
        uid: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
    };
    let authentication_json = serde_json::to_string(&authentication_data).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let _: Option<String> = valkey_client.set_ex(key, authentication_json, 300).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = StartPasskeyAuthenticationResponse { challenge, flow_id };
    Ok((
        sign_response(format!("passkey_start-{}", BASE64_STANDARD.encode(Sha512::digest(body))), Some(&response))?,
        Json(response),
    ))
}
pub async fn finish_passkey_authentication(body: Bytes) -> Result<(HeaderMap, Json<FinishPasskeyAuthenticationResponse>), StatusCode> {
    let request: FinishPasskeyAuthenticationRequest = serde_json::from_str(&decrypt_body(body.clone())?).or(Err(StatusCode::BAD_REQUEST))?;
    let webauthn = get_webauthn()?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let key = format!("PASSKEY_AUTHENTICATION:{}", request.flow_id);
    let authentication_json: Option<String> = valkey_client.get_del(key).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let authentication: PasskeyAuthenticationValkey = serde_json::from_str(&authentication_json.ok_or(StatusCode::UNAUTHORIZED)?).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let result = webauthn
        .finish_passkey_authentication(&request.credential, &authentication.authentication)
        .or(Err(StatusCode::UNAUTHORIZED))?;
    let mut db = get_db()?;
    let user: User = users::table.find(authentication.uid).get_result(&mut db).or(Err(StatusCode::UNAUTHORIZED))?;
    let _: Passkey = passkeys::table
        .filter(passkeys::uid.eq(authentication.uid).and(passkeys::id.eq(result.cred_id().to_vec())))
        .first(&mut db)
        .or(Err(StatusCode::UNAUTHORIZED))?;
    let jwt = generate_jwt("webserver", &user, JWT_EXPIRY, Some(request.pubkey))?;
    let response = FinishPasskeyAuthenticationResponse { jwt };
    Ok((
        sign_response(format!("passkey_finish-{}", BASE64_STANDARD.encode(Sha512::digest(body))), Some(&response))?,
        Json(response),
    ))
}
