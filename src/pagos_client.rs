use error_stack::Report;
use masking::{ExposeInterface, Maskable, Secret};

use crate::{
    api_client::{ApiClient, Headers as ApiClientHeaders, Method as ApiMethod},
    app::TenantAppState,
    error, logger,
    types::pagos::PagosPanDetailsResponse,
    utils::CustomResult,
};

#[derive(Clone)]
pub struct PagosApiClient {
    api_client: ApiClient,
    base_url: String,
    api_key: Secret<String>,
}

impl PagosApiClient {
    pub fn new(api_client: ApiClient, base_url: String, api_key: Secret<String>) -> Self {
        Self {
            api_client,
            base_url,
            api_key,
        }
    }

    pub async fn get_pan_details(
        &self,
        bin_number: &str,
    ) -> Result<PagosPanDetailsResponse, Report<error::ApiClientError>> {
        let url = format!("{}/bins?enhanced=true&bin={}", self.base_url, bin_number);

        let mut api_headers = ApiClientHeaders::new();
        api_headers.insert((
            "x-api-key".to_string(),
            Maskable::from(self.api_key.clone().expose()),
        ));
        api_headers.insert((
            "Accept".to_string(),
            Maskable::from("application/json".to_string()),
        ));

        self.api_client
            .send_request(url, api_headers, ApiMethod::Get, ())
            .await
            .map_err(|container_error| {
                logger::error!(api_client_error = ?container_error, "Pagos API call failed");
                container_error.error
            })
    }
}

pub async fn fetch_pan_details_internal(
    app_state: &TenantAppState,
    card_isin: &str,
) -> CustomResult<PagosPanDetailsResponse, error::ApiError> {
    let pagos_client = app_state.pagos_client.as_ref().ok_or_else(|| {
        Report::new(error::ApiError::ParsingError(
            "Pagos API client not initialized for tenant",
        ))
        .attach_printable(
            "Pagos API client not available in TenantAppState when fetching PAN details",
        )
    })?;

    pagos_client.get_pan_details(card_isin).await.map_err(|api_client_report: Report<error::ApiClientError>| { // Changed to error::
        logger::error!(error = ?api_client_report, card_isin = %card_isin, "Pagos get_pan_details failed");
        Report::new(error::ApiError::UnknownError)
            .attach_printable(format!("Pagos client error via ApiClient: {:?}", api_client_report))
    })
}
