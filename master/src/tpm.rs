use p384::{
    FieldBytes, Sec1Point,
    elliptic_curve::bigint::{ArrayEncoding, U384},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha384};
use signature::Verifier;
use ssh_key::{PublicKey, Signature, public::EcdsaPublicKey};
use std::{error::Error, str::FromStr};
use tss_esapi::{
    Context, WrapperErrorKind,
    attributes::ObjectAttributesBuilder,
    constants::{
        SessionType,
        tss::{TPM2_RH_NULL, TPM2_ST_HASHCHECK},
    },
    handles::{KeyHandle, SessionHandle},
    interface_types::{
        algorithm::{HashingAlgorithm, PublicAlgorithm},
        ecc::EccCurve,
        key_bits::RsaKeyBits,
        resource_handles::Hierarchy,
        session_handles::PolicySession,
    },
    structures::{
        Digest, EccPoint, EccScheme, HashScheme, HashcheckTicket, PcrSelectionListBuilder, PcrSlot, Public, PublicBuilder, PublicEccParametersBuilder, RsaExponent, Signature as TPMSignature,
        SignatureScheme, SymmetricDefinition, SymmetricDefinitionObject,
    },
    tcti_ldr::TctiNameConf,
    traits::{Marshall, UnMarshall},
    tss2_esys::TPMT_TK_HASHCHECK,
    utils::create_restricted_decryption_rsa_public,
};

pub struct TPMContext {
    context: Context,
    root_key: KeyHandle,
}
#[derive(Serialize, Deserialize)]
struct KeyMaterial {
    private: Vec<u8>,
    public: Vec<u8>,
}

pub fn tpm_context() -> Result<TPMContext, Box<dyn Error>> {
    let mut context = Context::new(TctiNameConf::from_str("device:/dev/tpmrm0")?)?;
    let root_key = context.execute_with_nullauth_session::<_, _, tss_esapi::Error>(|context| {
        Ok(context
            .create_primary(
                Hierarchy::Owner,
                create_restricted_decryption_rsa_public(SymmetricDefinitionObject::AES_256_CFB, RsaKeyBits::Rsa2048, RsaExponent::ZERO_EXPONENT)?,
                None,
                None,
                None,
                None,
            )?
            .key_handle)
    })?;
    Ok(TPMContext { context, root_key })
}
fn key_template(policy: Digest) -> tss_esapi::Result<Public> {
    PublicBuilder::new()
        .with_public_algorithm(PublicAlgorithm::Ecc)
        .with_name_hashing_algorithm(HashingAlgorithm::Sha256)
        .with_object_attributes(
            ObjectAttributesBuilder::new()
                .with_fixed_tpm(true)
                .with_fixed_parent(true)
                .with_sensitive_data_origin(true)
                .with_sign_encrypt(true)
                .build()?,
        )
        .with_auth_policy(policy)
        .with_ecc_parameters(PublicEccParametersBuilder::new_unrestricted_signing_key(EccScheme::EcDsa(HashScheme::new(HashingAlgorithm::Sha384)), EccCurve::NistP384).build()?)
        .with_ecc_unique_identifier(EccPoint::default())
        .build()
}
fn with_pcr_policy<T>(context: &mut Context, operation: impl FnOnce(&mut Context, PolicySession) -> tss_esapi::Result<T>) -> tss_esapi::Result<T> {
    let session = context
        .start_auth_session(None, None, None, SessionType::Policy, SymmetricDefinition::Null, HashingAlgorithm::Sha256)?
        .ok_or(tss_esapi::Error::WrapperError(WrapperErrorKind::WrongValueFromTpm))?;
    context.execute_with_temporary_object(SessionHandle::from(session).into(), |context, _| {
        let policy_session = PolicySession::try_from(session)?;
        let selection = PcrSelectionListBuilder::new().with_selection(HashingAlgorithm::Sha256, &[PcrSlot::Slot7]).build()?;
        context.policy_pcr(policy_session, Digest::default(), selection)?;
        operation(context, policy_session)
    })
}
fn field_bytes_from_slice(value: &[u8]) -> FieldBytes {
    U384::from_be_slice_truncated(value, 384).to_be_byte_array()
}
fn ssh_public_key(public: &Public) -> Result<PublicKey, Box<dyn Error>> {
    let unique = match public {
        Public::Ecc { unique, .. } => unique,
        _ => return Err("Invalid public key".into()),
    };

    let point = Sec1Point::from_affine_coordinates(&field_bytes_from_slice(unique.x().value()), &field_bytes_from_slice(unique.y().value()), false);
    Ok(EcdsaPublicKey::from_sec1_bytes(point.as_bytes())?.into())
}
pub fn tpm_generate_ssh_key(context: &mut TPMContext) -> Result<(String, String), Box<dyn Error>> {
    let policy = with_pcr_policy(&mut context.context, |context, session| context.policy_get_digest(session))?;
    let key = context
        .context
        .execute_with_nullauth_session::<_, _, tss_esapi::Error>(|tpm| tpm.create(context.root_key, key_template(policy)?, None, None, None, None))?;
    let public_key = ssh_public_key(&key.out_public)?.to_openssh()?;
    let material = KeyMaterial {
        private: key.out_private.value().to_vec(),
        public: key.out_public.marshall()?,
    };
    Ok((serde_json::to_string(&material)?, public_key))
}
pub fn tpm_sign_ssh(context: &mut TPMContext, key: &str, data: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let material: KeyMaterial = serde_json::from_str(key)?;
    let public = Public::unmarshall(&material.public)?;
    let public_key = ssh_public_key(&public)?;
    let digest = Digest::try_from(Sha384::digest(data).to_vec())?;
    let key_handle = context
        .context
        .execute_with_nullauth_session::<_, _, tss_esapi::Error>(|tpm| tpm.load(context.root_key, material.private.try_into()?, public))?;
    let signature = context.context.execute_with_temporary_object(key_handle.into(), |tpm, _| {
        let validation = HashcheckTicket::try_from(TPMT_TK_HASHCHECK {
            tag: TPM2_ST_HASHCHECK,
            hierarchy: TPM2_RH_NULL,
            digest: Default::default(),
        })?;
        with_pcr_policy(tpm, |tpm, session| {
            tpm.execute_with_session(Some(session.into()), |tpm| tpm.sign(key_handle, digest, SignatureScheme::Null, validation))
        })
    })?;
    let signature = match signature {
        TPMSignature::EcDsa(signature) => p384::ecdsa::Signature::from_scalars(field_bytes_from_slice(signature.signature_r().value()), field_bytes_from_slice(signature.signature_s().value()))?,
        _ => return Err("Invalid signature".into()),
    };
    let signature = Signature::try_from(signature)?;
    public_key.key_data().verify(data, &signature)?;
    Ok(signature.as_bytes().to_vec())
}
