use crate::{
    cerds,
    ctaphid::{CtapStatus, TransportError, ctaphid::Ctaphid},
};
use ctap_types::{ctap2::get_assertion, serde::cbor_serialize_to};
use ctaphid_types::{Channel, Command};
use log::debug;
use passkeyd_abi::config::Config;

pub fn handle(
    hid: &mut Ctaphid,
    config: &Config,
    channel: Channel,
    req: get_assertion::Request<'_>,
    raw_cbor: &[u8],
) -> anyhow::Result<()> {
    let response = cerds::get::get(hid, channel, config, req);
    let mut report = [0u8; size_of::<get_assertion::Response>() + 1]; //approx size.

    match response {
        Ok(res) => {
            let size = cbor_serialize_to(&res, &mut report[1..])?;
            let final_cbor = &mut report[..size + 1];
            hid.send_response(channel, Command::Cbor, final_cbor)?;
            debug!("Acknowledged ctab instruction 'GetAssertion'");
        }
        Err(e) => {
            if let Some(err) = e.downcast_ref::<CtapStatus>() {
                if *err == CtapStatus::NoCredentials {
                    log::info!("No local credentials found on token. Falling back to caBLE hybrid transport...");
                    match passkeyd_cable::perform_hybrid_assertion(raw_cbor) {
                        Ok(phone_cbor) => {
                            let mut final_cbor = Vec::with_capacity(phone_cbor.len() + 1);
                            final_cbor.push(0x00); // Prepend CTAP2_OK status byte
                            final_cbor.extend_from_slice(&phone_cbor);
                            hid.send_response(channel, Command::Cbor, &final_cbor)?;
                            log::info!("Acknowledged CTAP instruction 'GetAssertion' via caBLE hybrid transport");
                            return Ok(());
                        }
                        Err(cable_err) => {
                            log::warn!("caBLE hybrid assertion failed: {:?}", cable_err);
                        }
                    }
                }
                report[0] = *err as u8;
                hid.send_64response(channel, Command::Cbor, &report[0..1])?;
            } else if let Some(err) = e.downcast_ref::<TransportError>() {
                hid.send_portocal_error(err.channel, err.err)?;
            } else {
                report[0] = CtapStatus::Other as u8;
                hid.send_64response(channel, Command::Cbor, &report[0..1])?;
            }
        }
    };

    Ok(())
}
