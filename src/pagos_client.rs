use reqwest::header;

use crate::{
    app::TenantAppState, error::ApiError, logger, types::pagos::PagosPanDetailsResponse,
    utils::CustomResult,
};

type PagosClientResult<T> = Result<T, error_stack::Report<PagosClientError>>;

#[derive(Debug, thiserror::Error)]
pub enum PagosClientError {
    #[error("HTTP request failed")]
    NetworkError(#[from] reqwest::Error),
    #[error("Failed to deserialize Pagos API response")]
    DeserializationError(#[from] serde_json::Error),
    #[error("Pagos API returned an error: {status} - {body}")]
    ApiError {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

#[derive(Clone)]
pub struct PagosApiClient {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl PagosApiClient {
    pub fn new(base_url: String, api_key: String) -> PagosClientResult<Self> {
        let client = reqwest::Client::builder()
            .build()
            .map_err(PagosClientError::NetworkError)?;
        Ok(Self {
            client,
            base_url,
            api_key,
        })
    }

    pub async fn get_pan_details(
        &self,
        bin_number: &str,
    ) -> PagosClientResult<PagosPanDetailsResponse> {
        let url = format!("{}/bins?enhanced=true&bin={}", self.base_url, bin_number);

        let mut headers = header::HeaderMap::new();
        headers.insert(
            "x-api-key",
            header::HeaderValue::from_str(&self.api_key).map_err(|_| {
                error_stack::Report::new(PagosClientError::ConfigError(
                    "Invalid Pagos API key format".to_string(),
                ))
            })?,
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );

        let response = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(PagosClientError::NetworkError)?;

        if response.status().is_success() {
            let body_text = response
                .text()
                .await
                .map_err(PagosClientError::NetworkError)?;

            Ok(serde_json::from_str::<PagosPanDetailsResponse>(&body_text)
                .map_err(PagosClientError::DeserializationError)?)
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            logger::error!(pagos_api_error_status = %status, pagos_api_error_body = %body, "Pagos API error");
            Err(error_stack::Report::new(PagosClientError::ApiError {
                status,
                body,
            }))
        }
    }
}

pub async fn fetch_pan_details_internal(
    app_state: &TenantAppState,
    card_isin: &str,
) -> CustomResult<PagosPanDetailsResponse, ApiError> {
    let pagos_client = app_state.pagos_client.as_ref().ok_or_else(|| {
        error_stack::Report::new(ApiError::ParsingError(
            "Pagos API client not initialized for tenant",
        ))
        .attach_printable(
            "Pagos API client not available in TenantAppState when fetching PAN details",
        )
    })?;

    pagos_client.get_pan_details(card_isin).await.map_err(|e| {
        logger::error!(error = %e, card_isin = %card_isin, "Failed to fetch PAN details from Pagos internally");
        error_stack::Report::new(ApiError::UnknownError)
            .attach_printable(format!("Pagos client error: {:?}", e))
    })
}
