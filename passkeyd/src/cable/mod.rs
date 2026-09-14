use anyhow::Result;
use log::info;
use webauthn_authenticator_rs::{
    cable::connect_cable_tunnel,
    types::CableRequestType,
    ui::Cli,
};

pub enum CableRequest<'a> {
    GetAssertion(&'a [u8]),
    MakeCredential(&'a [u8]),
}

/// Performs a synchronous hybrid transport (caBLE v2) passkey exchange.
/// Scopes its own multi-threaded Tokio runtime to avoid imposing async runtime constraints
/// on the caller or main daemon loop.
pub fn perform_hybrid_assertion(raw_ctap_request: &[u8]) -> Result<Vec<u8>> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    rt.block_on(async move {
        info!("Starting caBLE v2 hybrid assertion session...");
        let ui = Cli {};
        let mut tunnel = connect_cable_tunnel(CableRequestType::GetAssertion, &ui).await
            .map_err(|e| anyhow::anyhow!("caBLE tunnel connection failed: {:?}", e))?;

        info!("caBLE tunnel connected! Transmitting CTAP assertion request ({} bytes)...", raw_ctap_request.len());
        let response = tunnel.transmit_cbor(raw_ctap_request, &ui).await
            .map_err(|e| anyhow::anyhow!("caBLE CTAP assertion transmission failed: {:?}", e))?;

        info!("Received signed CTAP assertion response ({} bytes) from phone", response.len());
        Ok(response)
    })
}

pub fn perform_hybrid_make_credential(raw_ctap_request: &[u8]) -> Result<Vec<u8>> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;

    rt.block_on(async move {
        info!("Starting caBLE v2 hybrid make_credential session...");
        let ui = Cli {};
        let mut tunnel = connect_cable_tunnel(CableRequestType::MakeCredential, &ui).await
            .map_err(|e| anyhow::anyhow!("caBLE tunnel connection failed: {:?}", e))?;

        info!("caBLE tunnel connected! Transmitting CTAP make_credential request ({} bytes)...", raw_ctap_request.len());
        let response = tunnel.transmit_cbor(raw_ctap_request, &ui).await
            .map_err(|e| anyhow::anyhow!("caBLE CTAP make_credential transmission failed: {:?}", e))?;

        info!("Received new CTAP credential response ({} bytes) from phone", response.len());
        Ok(response)
    })
}

pub fn format_ctap_cbor_response(payload: &[u8]) -> Vec<u8> {
    let mut response = Vec::with_capacity(payload.len() + 1);
    response.push(0x00); // Prepend CTAP2_OK status byte
    response.extend_from_slice(payload);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ctap_cbor_response_prepends_status() {
        let dummy_cbor = [0xa1, 0x01, 0x02];
        let framed = format_ctap_cbor_response(&dummy_cbor);
        assert_eq!(framed.len(), dummy_cbor.len() + 1);
        assert_eq!(framed[0], 0x00); // CTAP2_OK
        assert_eq!(&framed[1..], &dummy_cbor);
    }

    #[test]
    fn test_scoped_tokio_runtime_lifecycle() {
        for i in 0..3 {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("Failed to build scoped Tokio runtime");

            let result = rt.block_on(async move {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                i * 10
            });

            assert_eq!(result, i * 10);
        }
    }
}
