use crate::{
    auth::{JWT_EXPIRY, authenticate, generate_jwt},
    get_db,
    step_up::{StepUpVerification, verify_step_up},
    util::{convert_timestamp, decrypt_body, sign_response, validate_signature},
};
use axum::{
    Json,
    body::Bytes,
    extract::{OriginalUri, Path},
    http::{HeaderMap, StatusCode},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD, prelude::BASE64_STANDARD};
use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use redis::AsyncTypedCommands;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use shared_server::{
    get_valkey_client,
    models::{Passkey, Setting, User},
    notify_server,
    schema::{passkeys, settings, users},
};
use std::{env, time::SystemTime};
use uuid::Uuid;
use webauthn_rs::{
    Webauthn, WebauthnBuilder,
    fake::{FakePasskeyDistribution, WebauthnFakeCredentialGenerator},
    prelude::{CreationChallengeResponse, PasskeyAuthentication, PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential, RequestChallengeResponse, Url},
};
use webauthn_rs_core::proto::{AllowCredentials, CredentialID};

#[derive(Deserialize)]
pub struct StartPasskeyRegistrationRequest {
    verification: StepUpVerification,
}
#[derive(Deserialize)]
pub struct FinishPasskeyRegistrationRequest {
    name: String,
    credential: RegisterPublicKeyCredential,
}
#[derive(Deserialize)]
pub struct StartPasskeyAuthenticationRequest {
    username: String,
}
#[derive(Serialize)]
pub struct StartPasskeyAuthenticationResponse {
    pub challenge: RequestChallengeResponse,
    pub flow_id: Uuid,
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
    uid: Option<Uuid>,
}
#[derive(Serialize)]
pub struct APIPasskey {
    id: String,
    created_at: u64,
    last_used: Option<u64>,
    name: String,
}

pub async fn get_webauthn(db: &mut AsyncPgConnection) -> Result<Webauthn, StatusCode> {
    let web_host = env::var("WEBSERVER_HOST").or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let url = Url::parse(&format!("https://{}", web_host)).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let apk_hash: Setting = settings::table.find("apk_hash").get_result(db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut builder = WebauthnBuilder::new(&web_host, &url).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    if !apk_hash.value.is_empty() {
        let apk_hash = apk_hash.value.replace(":", "");
        let apk_hash = hex::decode(apk_hash).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        let apk_hash = URL_SAFE_NO_PAD.encode(apk_hash);
        let url = Url::parse(&format!("android:apk-key-hash:{}", apk_hash)).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        builder = builder.append_allowed_origin(&url);
    }
    builder.build().or(Err(StatusCode::INTERNAL_SERVER_ERROR))
}
pub async fn start_passkey_registration(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<CreationChallengeResponse>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: StartPasskeyRegistrationRequest = serde_json::from_slice(&decrypt_body(&headers, body).await?).or(Err(StatusCode::BAD_REQUEST))?;
    verify_step_up(&headers, &user, request.verification).await?;
    let mut db = get_db().await?;
    let webauthn = get_webauthn(&mut db).await?;
    let passkeys_db: Vec<Passkey> = passkeys::table
        .filter(passkeys::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .load(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let credential_ids = passkeys_db.iter().map(|i| CredentialID::from(i.id.clone())).collect();
    let (challenge, registration) = webauthn
        .start_passkey_registration(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?, &user.username, &user.username, Some(credential_ids))
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let key = format!("PASSKEY_REGISTRATION:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?);
    let registration_json = serde_json::to_string(&registration).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    valkey_client.set_ex(key, registration_json, 300).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    Ok((sign_response(signature, Some(&challenge))?, Json(challenge)))
}
pub async fn finish_passkey_registration(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "POST").await?;
    let request: FinishPasskeyRegistrationRequest = serde_json::from_slice(&decrypt_body(&headers, body).await?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let key = format!("PASSKEY_REGISTRATION:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?);
    let registration_json: Option<String> = valkey_client.get_del(key).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let registration: PasskeyRegistration = match registration_json {
        Some(json) => serde_json::from_str(&json).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?,
        None => return Err(StatusCode::BAD_REQUEST),
    };
    let mut db = get_db().await?;
    let webauthn = get_webauthn(&mut db).await?;
    let passkey = webauthn.finish_passkey_registration(&request.credential, &registration).or(Err(StatusCode::BAD_REQUEST))?;
    let passkey_json = serde_json::to_string(&passkey).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let passkey_db = Passkey {
        id: passkey.cred_id().to_vec(),
        uid: user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?,
        passkey: passkey_json,
        created_at: SystemTime::now(),
        last_used: None,
        name: request.name,
    };
    diesel::insert_into(passkeys::table).values(passkey_db).execute(&mut db).await.or(Err(StatusCode::BAD_REQUEST))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
pub async fn start_passkey_authentication(headers: HeaderMap, body: Bytes) -> Result<(HeaderMap, Json<StartPasskeyAuthenticationResponse>), StatusCode> {
    let request: StartPasskeyAuthenticationRequest = serde_json::from_slice(&decrypt_body(&headers, body.clone()).await?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut db = get_db().await?;
    let webauthn = get_webauthn(&mut db).await?;
    let user_option: Option<User> = users::table
        .filter(users::username.eq(&request.username).and(users::enabled.eq(true)))
        .first(&mut db)
        .await
        .optional()
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let (uid, passkeys) = match user_option {
        Some(user) => {
            let passkeys = load_authentication_passkeys(&mut db, user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?).await?;
            (user.id, passkeys)
        }
        None => (None, vec![]),
    };
    let (mut challenge, authentication) = webauthn.start_passkey_authentication(&passkeys).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    if uid.is_none() {
        let fake_passkey_key: Setting = settings::table.find("fake_passkey_key").get_result(&mut db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        let key = BASE64_STANDARD.decode(fake_passkey_key.value).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        let fake_credential_ids = WebauthnFakeCredentialGenerator::<FakePasskeyDistribution>::new(&key)
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
            .generate(&request.username.into_bytes())
            .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        challenge.public_key.allow_credentials = fake_credential_ids
            .iter()
            .map(|id| AllowCredentials {
                id: id.as_ref().into(),
                type_: "public-key".to_string(),
                transports: None,
            })
            .collect();
    }
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let flow_id = Uuid::new_v4();
    let key = format!("PASSKEY_AUTHENTICATION:{}", flow_id);
    let authentication_data = PasskeyAuthenticationValkey { authentication, uid };
    let authentication_json = serde_json::to_string(&authentication_data).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    valkey_client.set_ex(key, authentication_json, 300).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let response = StartPasskeyAuthenticationResponse { challenge, flow_id };
    Ok((
        sign_response(format!("passkey_start-{}", BASE64_STANDARD.encode(Sha512::digest(body))), Some(&response))?,
        Json(response),
    ))
}
pub async fn finish_passkey_authentication(headers: HeaderMap, body: Bytes) -> Result<(HeaderMap, Json<FinishPasskeyAuthenticationResponse>), StatusCode> {
    let request: FinishPasskeyAuthenticationRequest = serde_json::from_slice(&decrypt_body(&headers, body.clone()).await?).or(Err(StatusCode::BAD_REQUEST))?;
    let mut valkey_client = get_valkey_client().await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?.0;
    let key = format!("PASSKEY_AUTHENTICATION:{}", request.flow_id);
    let authentication_json: Option<String> = valkey_client.get_del(key).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let authentication: PasskeyAuthenticationValkey = serde_json::from_str(&authentication_json.ok_or(StatusCode::UNAUTHORIZED)?).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let uid = authentication.uid.ok_or(StatusCode::UNAUTHORIZED)?;
    let mut db = get_db().await?;
    let user: User = users::table.find(uid).get_result(&mut db).await.or(Err(StatusCode::UNAUTHORIZED))?;
    verify_passkey_authentication(&mut db, uid, &request.credential, &authentication.authentication, StatusCode::UNAUTHORIZED).await?;
    diesel::update(users::table.find(uid))
        .set(users::last_login.eq(Some(SystemTime::now())))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server("WEB:REFRESH").await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let jwt = generate_jwt("webserver", &user, JWT_EXPIRY, Some(request.pubkey), None, &mut db).await?;
    let response = FinishPasskeyAuthenticationResponse { jwt };
    Ok((
        sign_response(format!("passkey_finish-{}", BASE64_STANDARD.encode(Sha512::digest(body))), Some(&response))?,
        Json(response),
    ))
}
pub async fn load_authentication_passkeys(db: &mut AsyncPgConnection, uid: Uuid) -> Result<Vec<webauthn_rs::prelude::Passkey>, StatusCode> {
    let credentials: Vec<Passkey> = passkeys::table.filter(passkeys::uid.eq(uid)).load(db).await.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let mut passkeys = vec![];
    for i in credentials {
        let passkey: webauthn_rs::prelude::Passkey = serde_json::from_str(&i.passkey).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
        passkeys.push(passkey);
    }
    Ok(passkeys)
}
pub async fn verify_passkey_authentication(
    db: &mut AsyncPgConnection,
    uid: Uuid,
    credential: &PublicKeyCredential,
    authentication: &PasskeyAuthentication,
    error: StatusCode,
) -> Result<(), StatusCode> {
    let result = get_webauthn(db).await?.finish_passkey_authentication(credential, authentication).or(Err(error))?;
    let credential: Passkey = passkeys::table
        .filter(passkeys::uid.eq(uid).and(passkeys::id.eq(result.cred_id().to_vec())))
        .first(db)
        .await
        .or(Err(error))?;
    let mut passkey: webauthn_rs::prelude::Passkey = serde_json::from_str(&credential.passkey).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    passkey.update_credential(&result);
    diesel::update(passkeys::table.filter(passkeys::uid.eq(credential.uid).and(passkeys::id.eq(credential.id))))
        .set((
            passkeys::passkey.eq(serde_json::to_string(&passkey).or(Err(StatusCode::INTERNAL_SERVER_ERROR))?),
            passkeys::last_used.eq(Some(SystemTime::now())),
        ))
        .execute(db)
        .await
        .or(Err(error))?;
    Ok(())
}
pub async fn passkeys_list(headers: HeaderMap, uri: OriginalUri, body: Bytes) -> Result<(HeaderMap, Json<Vec<APIPasskey>>), StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "GET").await?;
    let mut db = get_db().await?;
    let passkeys: Vec<Passkey> = passkeys::table
        .filter(passkeys::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .get_results(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    let passkeys: Result<Vec<APIPasskey>, StatusCode> = passkeys
        .iter()
        .map(|passkey| {
            Ok(APIPasskey {
                id: hex::encode(&passkey.id),
                created_at: convert_timestamp(passkey.created_at)?,
                last_used: passkey.last_used.map(|timestamp| convert_timestamp(timestamp).unwrap_or(0)),
                name: passkey.name.clone(),
            })
        })
        .collect();
    let passkeys = passkeys?;
    Ok((sign_response(signature, Some(&passkeys))?, Json(passkeys)))
}
pub async fn delete_passkey(headers: HeaderMap, Path(id): Path<String>, uri: OriginalUri, body: Bytes) -> Result<HeaderMap, StatusCode> {
    let user = authenticate(&headers).await?.0;
    let signature = validate_signature(&headers, uri.0, &body, "DELETE").await?;
    let mut db = get_db().await?;
    let credential_id = hex::decode(id).or(Err(StatusCode::BAD_REQUEST))?;
    diesel::delete(passkeys::table.filter(passkeys::id.eq(credential_id).and(passkeys::uid.eq(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::NOT_FOUND))?;
    diesel::update(users::table.find(user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .set(users::jwt_counter.eq(user.jwt_counter + 1))
        .execute(&mut db)
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    notify_server(&format!("WEB:REFRESH:{}", user.id.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?))
        .await
        .or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
    sign_response::<String>(signature, None)
}
