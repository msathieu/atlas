use p384::{FieldBytes, elliptic_curve::bigint::U384};
use sha2::{Digest as _, Sha384};
use signature::Verifier;
use ssh_key::{PublicKey, Signature, public::EcdsaPublicKey};
use std::{error::Error, str::FromStr};
use tss_esapi::{
    abstraction::transient::{KeyMaterial, KeyParams, TransientKeyContext},
    interface_types::{algorithm::HashingAlgorithm, ecc::EccCurve},
    structures::{Digest, EccScheme, HashScheme, Signature as TPMSignature},
    tcti_ldr::TctiNameConf,
    utils::PublicKey as TPMPublicKey,
};

pub fn tpm_context() -> Result<TransientKeyContext, Box<dyn Error>> {
    Ok(TransientKeyContext::builder().with_tcti(TctiNameConf::from_str("device:/dev/tpmrm0")?).build()?)
}
fn key_params() -> KeyParams {
    KeyParams::Ecc {
        curve: EccCurve::NistP384,
        scheme: EccScheme::EcDsa(HashScheme::new(HashingAlgorithm::Sha384)),
    }
}
fn field_bytes_from_slice(value: &[u8]) -> Result<FieldBytes, Box<dyn Error>> {
    Ok(FieldBytes::try_from(&U384::from_be_slice_truncated(value, 384).to_be_bytes().to_vec())?)
}
fn ssh_public_key(public: &TPMPublicKey) -> Result<PublicKey, Box<dyn Error>> {
    let (x, y) = match public {
        TPMPublicKey::Ecc { x, y } => (x, y),
        _ => return Err("Invalid public key".into()),
    };
    let mut point = vec![4];
    point.extend_from_slice(&field_bytes_from_slice(x)?);
    point.extend_from_slice(&field_bytes_from_slice(y)?);
    Ok(PublicKey::new(EcdsaPublicKey::from_sec1_bytes(&point)?.into(), ""))
}
pub fn tpm_generate_ssh_key(context: &mut TransientKeyContext) -> Result<(String, String), Box<dyn Error>> {
    let (material, _) = context.create_key(key_params(), 0)?;
    Ok((serde_json::to_string(&material)?, ssh_public_key(material.public())?.to_openssh()?))
}
pub fn tpm_sign_ssh(context: &mut TransientKeyContext, key: &str, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let material: KeyMaterial = serde_json::from_str(key)?;
    let public_key = ssh_public_key(material.public())?;
    let digest = Digest::try_from(Sha384::digest(data).to_vec())?;
    let signature = context.sign(material, key_params(), None, digest)?;
    let signature = match signature {
        TPMSignature::EcDsa(signature) => p384::ecdsa::Signature::from_scalars(field_bytes_from_slice(signature.signature_r().value())?, field_bytes_from_slice(signature.signature_s().value())?)?,
        _ => return Err("Invalid signature".into()),
    };
    let signature = Signature::try_from(signature)?;
    public_key.key_data().verify(data, &signature)?;
    Ok(signature.as_bytes().to_vec())
}
