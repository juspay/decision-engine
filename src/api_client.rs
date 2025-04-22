use std::str::FromStr;

use crate::{
    config::GlobalConfig,
    error::{self, ResultContainerExt},
};
use masking::Maskable;
use reqwest::StatusCode;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Response,
};

pub type Headers = std::collections::HashSet<(String, Maskable<String>)>;

pub(super) trait HeaderExt {
    fn construct_header_map(
        self,
    ) -> Result<HeaderMap, error::ContainerError<error::ApiClientError>>;
}

impl HeaderExt for Headers {
    fn construct_header_map(
        self,
    ) -> Result<HeaderMap, error::ContainerError<error::ApiClientError>> {
        self.into_iter().try_fold(
            HeaderMap::new(),
            |mut header_map, (header_name, header_value)| {
                let header_name = HeaderName::from_str(&header_name)
                    .change_error(error::ApiClientError::HeaderMapConstructionFailed)?;
                let header_value = header_value.into_inner();
                let header_value = HeaderValue::from_str(&header_value)
                    .change_error(error::ApiClientError::HeaderMapConstructionFailed)?;
                header_map.append(header_name, header_value);
                Ok(header_map)
            },
        )
    }
}

pub enum Method {
    Get,
    Post,
}

#[derive(Clone, serde::Deserialize, Debug)]
#[serde(default)]
pub struct ApiClientConfig {
    pub client_idle_timeout: u64,
    pub pool_max_idle_per_host: usize,
}

#[derive(Clone)]
pub struct ApiClient {
    pub inner: reqwest::Client,
}

impl std::ops::Deref for ApiClient {
    type Target = reqwest::Client;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl ApiClient {
    #[allow(unused_mut)]
    pub fn new(
        global_config: &GlobalConfig,
    ) -> Result<Self, error::ContainerError<error::ApiClientError>> {
        let mut client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .pool_idle_timeout(std::time::Duration::from_secs(
                global_config.api_client.client_idle_timeout,
            ))
            .pool_max_idle_per_host(global_config.api_client.pool_max_idle_per_host);

        let client = client
            .build()
            .change_error(error::ApiClientError::ClientConstructionFailed)?;

        Ok(Self { inner: client })
    }

    pub async fn send_request<T>(
        &self,
        url: String,
        headers: Headers,
        method: Method,
        request_body: T,
    ) -> Result<ApiResponse, error::ContainerError<error::ApiClientError>>
    where
        T: serde::Serialize + Send + Sync + 'static,
    {
        let url =
            reqwest::Url::parse(&url).change_error(error::ApiClientError::UrlEncodingFailed)?;

        let headers = headers.construct_header_map()?;

        let request_builder = match method {
            Method::Get => self.get(url),
            Method::Post => self.post(url).json(&request_body),
        };

        let response = request_builder
            .headers(headers)
            .send()
            .await
            .change_error(error::ApiClientError::RequestNotSent)?;

        match response.status() {
            StatusCode::OK => Ok(ApiResponse(response)),
            StatusCode::INTERNAL_SERVER_ERROR => Err(error::ApiClientError::InternalServerError(
                response
                    .bytes()
                    .await
                    .change_error(error::ApiClientError::ResponseDecodingFailed)?,
            )
            .into()),
            StatusCode::BAD_REQUEST => Err(error::ApiClientError::BadRequest(
                response
                    .bytes()
                    .await
                    .change_error(error::ApiClientError::ResponseDecodingFailed)?,
            )
            .into()),
            StatusCode::UNAUTHORIZED => Err(error::ApiClientError::Unauthorized(
                response
                    .bytes()
                    .await
                    .change_error(error::ApiClientError::ResponseDecodingFailed)?,
            )
            .into()),
            _ => Err(error::ApiClientError::Unexpected {
                status_code: response.status(),
                message: response
                    .bytes()
                    .await
                    .change_error(error::ApiClientError::ResponseDecodingFailed)?,
            }
            .into()),
        }
    }
}

pub struct ApiResponse(Response);

impl ApiResponse {
    pub async fn deserialize_json<R, E>(self) -> Result<R, error::ContainerError<E>>
    where
        R: serde::de::DeserializeOwned,
        error::ContainerError<E>: From<error::ContainerError<error::ApiClientError>> + Send + Sync,
    {
        Ok(self
            .0
            .json::<R>()
            .await
            .change_error(error::ApiClientError::ResponseDecodingFailed)?)
    }
}
