use crate::models::Setting;
use crate::schema::settings;
use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use diesel::PgConnection;
use diesel::prelude::*;
use object::{Object, ObjectSection};
use std::{error::Error, ffi::CString, str::FromStr};
use tokio::fs;

pub async fn personalize_binary(db: &mut PgConnection, arch: &str, os: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let host: Setting = settings::table.find("apiserver_host").get_result(db)?;
    let host = host.value;
    let tls_cert: Setting = settings::table.find("tls_cert").get_result(db)?;
    let tls_cert = tls_cert.value;
    let registration_token: Setting = settings::table.find("registration_token").get_result(db)?;
    let registration_token = BASE64_STANDARD.decode(registration_token.value)?;
    if host.len() > 255 || tls_cert.len() > 1023 || registration_token.len() != 64 {
        return Err("Parameters are too long".into());
    }
    let mut content = fs::read(format!("agent-{}-{}", arch, os)).await?;
    let binary = object::File::parse(content.as_slice())?;

    let host_section = binary.section_by_name("apiserver_host").ok_or("Invalid binary")?;
    let host_range = host_section.file_range().ok_or("Invalid binary")?;
    let mut host = CString::from_str(&host)?.as_bytes_with_nul().to_vec();
    host.resize(host_range.1 as usize, 0);

    let cert_section = binary.section_by_name("apiserver_cert").ok_or("Invalid binary")?;
    let cert_range = cert_section.file_range().ok_or("Invalid binary")?;
    let mut tls_cert = CString::from_str(&tls_cert)?.as_bytes_with_nul().to_vec();
    tls_cert.resize(cert_range.1 as usize, 0);

    let token_section = binary.section_by_name("registration_token").ok_or("Invalid binary")?;
    let token_range = token_section.file_range().ok_or("Invalid binary")?;

    content.splice(host_range.0 as usize..host_range.0 as usize + host_range.1 as usize, host);
    content.splice(cert_range.0 as usize..cert_range.0 as usize + cert_range.1 as usize, tls_cert);
    content.splice(token_range.0 as usize..token_range.0 as usize + token_range.1 as usize, registration_token);
    Ok(content)
}
