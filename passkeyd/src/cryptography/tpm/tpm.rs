use anyhow::Context as AnyhowContex;
use log::debug;
use log::info;
use log::warn;
use sha2::{Digest as SHA2Digest, Sha256};
use std::str::FromStr;
use tss_esapi::{
    Context, TctiNameConf,
    attributes::ObjectAttributesBuilder,
    constants::{
        SessionType,
        tss::{TPM2_RH_NULL, TPM2_ST_HASHCHECK},
    },
    handles::{KeyHandle, ObjectHandle, SessionHandle},
    interface_types::{
        algorithm::{HashingAlgorithm, PublicAlgorithm, SymmetricMode},
        ecc::EccCurve,
        key_bits::AesKeyBits,
        resource_handles::Hierarchy,
        session_handles::AuthSession,
    },
    structures::{
        Auth, Digest, EccPoint, EccScheme, HashcheckTicket, KeyDerivationFunctionScheme,
        PcrSelectionListBuilder, PcrSlot, Private, Public, PublicBuilder,
        PublicEccParametersBuilder, Signature, SignatureScheme, SymmetricDefinition,
        SymmetricDefinitionObject,
    },
    tss2_esys::TPMT_TK_HASHCHECK,
};

// Some older firmware only measures into the SHA-1 bank and leaves the SHA-256
// bank allocated but at all zeros.The policy session itself always uses SHA-256 (authHash), and
// signatures are SHA-256 as well; only the PCR differs.
const PCR_BANK_PREFERENCE: [HashingAlgorithm; 2] =
    [HashingAlgorithm::Sha256, HashingAlgorithm::Sha1];

pub fn initialize_tpm_with_session() -> anyhow::Result<Context> {
    let mut ctx = initialize_tpm()?;
    debug!("TPM has been initialized");
    initialize_pass_session(&mut ctx);
    Ok(ctx)
}

fn initialize_tpm() -> anyhow::Result<Context> {
    let tcti = TctiNameConf::from_str("device:/dev/tpmrm0")?;
    let mut ctx = Context::new(tcti)?;
    ctx.startup(tss_esapi::constants::StartupType::Clear)?;
    Ok(ctx)
}

fn initialize_pass_session(ctx: &mut Context) {
    debug!("TPM session set to password.");
    ctx.set_sessions((Some(AuthSession::Password), None, None));
}

// some duct tape function
// has been promoted to
// New duct tape function
fn pcr7_session(
    ctx: &mut Context,
    session_type: SessionType,
    expected_policy: Option<&Digest>,
) -> anyhow::Result<(AuthSession, Digest)> {
    for bank in PCR_BANK_PREFERENCE {
        let selection = PcrSelectionListBuilder::new()
            .with_selection(bank, &[PcrSlot::Slot7])
            .build()
            .context("failed building the PCR selection")?;

        let values = match ctx.pcr_read(selection.clone()) {
            Ok((_, _, values)) => values,
            Err(e) if expected_policy.is_none() => {
                anyhow::bail!(
                    "Failed to read PCR 7 from the {bank:?} bank. Fallback is unavailable: {e}."
                );
            }
            Err(e) => {
                warn!("Failed to read PCR 7 from the {bank:?} bank, trying next bank: {e}");
                continue;
            }
        };

        let Some(pcr_value) = values
            .value()
            .first()
            .filter(|value| !value.value().iter().all(|byte| *byte == 0))
        else {
            info!("PCR 7 is empty in the {bank:?} bank. Trying fallback.");
            continue;
        };

        let pcr_digest = Digest::try_from(Sha256::digest(pcr_value.value()).as_slice())?;

        let session = ctx
            .start_auth_session(
                None,
                None,
                None,
                session_type,
                SymmetricDefinition::Aes {
                    key_bits: AesKeyBits::Aes128,
                    mode: SymmetricMode::Cfb,
                },
                HashingAlgorithm::Sha256,
            )
            .context("out of session slots?")?
            .context("the TPM returned no auth session")?;

        if let Err(e) = ctx.policy_pcr(session.try_into()?, pcr_digest, selection) {
            warn!("Applying PCR 7 policy for the {bank:?} bank failed: {e}");
            let _ = ctx.flush_context(ObjectHandle::from(SessionHandle::from(session)));
            continue;
        }

        let policy = match ctx.policy_get_digest(session.try_into()?) {
            Ok(digest) => digest,
            Err(e) => {
                warn!("Reading PCR 7 policy digest for the {bank:?} bank failed: {e}");
                let _ = ctx.flush_context(ObjectHandle::from(SessionHandle::from(session)));
                continue;
            }
        };
        if expected_policy.is_none_or(|expected| expected.value() == policy.value()) {
            if expected_policy.is_none() && bank == HashingAlgorithm::Sha1 {
                warn!(
                    "PCR 7 is not filled in the SHA-256 bank, Binding the key to \
                     the weaker SHA-1 bank as a fallback"
                );
            }
            debug!("PCR 7 policy built from the {bank:?} bank");
            return Ok((session, policy));
        }
        let _ = ctx.flush_context(ObjectHandle::from(SessionHandle::from(session)));
        debug!(
            "PCR 7 policy from the {bank:?} bank does not match the key's policy, trying next bank"
        );
    }

    match expected_policy {
        None => anyhow::bail!("PCR 7 is not extended in either the SHA-256 or SHA-1 bank."),
        Some(_) => anyhow::bail!(
            "PCR 7 policy mismatch, secure boot state or PCR bank allocation may have changed."
        ),
    }
}

fn initialize_policy_session(
    ctx: &mut Context,
    expected_policy: &Digest,
) -> anyhow::Result<AuthSession> {
    ctx.set_sessions((None, None, None));
    let (session, _) = pcr7_session(ctx, SessionType::Policy, Some(expected_policy))?;
    ctx.set_sessions((Some(session), None, None));
    debug!("TPM session set to PCR 7 policy.");
    Ok(session)
}

pub fn get_pcr7_policy(ctx: &mut Context) -> anyhow::Result<Digest> {
    let current_sessions = ctx.sessions();
    ctx.set_sessions((None, None, None));
    let result = pcr7_session(ctx, SessionType::Trial, None).map(|(session, digest)| {
        let _ = ctx.flush_context(ObjectHandle::from(SessionHandle::from(session)));
        digest
    });
    ctx.set_sessions(current_sessions);
    result
}

pub fn create_primary_key_handle(ctx: &mut Context) -> anyhow::Result<KeyHandle> {
    let ecc_params = PublicEccParametersBuilder::new()
        .with_curve(EccCurve::NistP256)
        .with_key_derivation_function_scheme(KeyDerivationFunctionScheme::Null)
        .with_ecc_scheme(EccScheme::Null)
        .with_symmetric(SymmetricDefinitionObject::Aes {
            key_bits: AesKeyBits::Aes128,
            mode: SymmetricMode::Cfb,
        })
        .with_restricted(true)
        .with_is_decryption_key(true)
        .with_is_signing_key(false)
        .build()
        .expect("failed to build ecc");

    let attributes = ObjectAttributesBuilder::new()
        .with_fixed_tpm(true)
        .with_fixed_parent(true)
        .with_sensitive_data_origin(true)
        .with_user_with_auth(true)
        .with_restricted(true)
        .with_decrypt(true)
        .with_sign_encrypt(false)
        .build()
        .expect("failed to build attributes");

    let public_template = PublicBuilder::new()
        .with_public_algorithm(PublicAlgorithm::Ecc)
        .with_ecc_parameters(ecc_params)
        .with_name_hashing_algorithm(HashingAlgorithm::Sha256)
        .with_object_attributes(attributes)
        .with_ecc_unique_identifier(EccPoint::default())
        .build()
        .expect("failed to build public template");

    let srk_handle = ctx.create_primary(
        Hierarchy::Owner,
        public_template,
        Some(Auth::default()),
        None,
        None,
        None,
    )?;
    debug!("Storage root key handler created");
    Ok(srk_handle.key_handle)
}

pub fn sign(
    ctx: &mut Context,
    srk_handle: &KeyHandle,
    private_key: Private,
    public_key: Public,
    data_to_sign: &[u8],
) -> anyhow::Result<Signature> {
    let expected_policy = public_key.auth_policy().clone();
    let digest = Digest::try_from(Sha256::digest(data_to_sign).as_slice())?;
    let validation = HashcheckTicket::try_from(TPMT_TK_HASHCHECK {
        hierarchy: TPM2_RH_NULL,
        tag: TPM2_ST_HASHCHECK,
        ..Default::default()
    })?;
    let key_handle = ctx.load(*srk_handle, private_key, public_key)?;

    let session = match initialize_policy_session(ctx, &expected_policy) {
        Ok(session) => session,
        Err(e) => {
            // Policy could not be satisfied: don't leave the key loaded.
            let _ = ctx.flush_context(ObjectHandle::from(key_handle));
            initialize_pass_session(ctx);
            return Err(e);
        }
    };
    let signature = ctx.sign(key_handle, digest, SignatureScheme::Null, validation);

    ctx.set_sessions((None, None, None));
    let _ = ctx.flush_context(ObjectHandle::from(SessionHandle::from(session)));
    let _ = ctx.flush_context(ObjectHandle::from(key_handle));
    initialize_pass_session(ctx);

    signature.map_err(|e| e.into())
}
