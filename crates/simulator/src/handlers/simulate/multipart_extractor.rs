use std::{collections::HashMap, sync::Arc};

use axum::extract::{FromRequest, Multipart, Request};
use bytes::Bytes;
use error_stack::ResultExt;

use crate::{
    app::AppState,
    errors::{ApiError, ContainerError},
    handlers::simulate::types::SimulateDataRequest,
};

#[derive(Debug, Clone)]
pub struct UploadDataResolver {
    pub upload_data: bool,
    pub json_data: SimulateDataRequest,
    pub csv_data: Option<Bytes>,
}

impl FromRequest<Arc<AppState>> for UploadDataResolver {
    type Rejection = ContainerError<ApiError>;

    async fn from_request(req: Request, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let query_params: HashMap<String, String> = req
            .uri()
            .query()
            .unwrap_or("")
            .split('&')
            .filter_map(|pair| {
                let mut split = pair.splitn(2, '=');
                Some((
                    split.next()?.to_string(),
                    split.next().unwrap_or("").to_string(),
                ))
            })
            .collect();

        let upload_data = query_params
            .get("upload_data")
            .map(|v| v == "true")
            .unwrap_or(false);

        let (csv_data, json_data) = if upload_data {
            let multipart = Multipart::from_request(req, state)
                .await
                .map_err(|_| ApiError::MultipartError("Missing multipart data"))?;

            let mut csv_data = Bytes::new();
            let mut json_data = Bytes::new();
            let mut multipart_stream = multipart;

            while let Some(field) = multipart_stream
                .next_field()
                .await
                .map_err(|_| ApiError::MultipartError("Error reading multipart field"))?
            {
                if let Some(filename) = field.file_name() {
                    if filename.ends_with(".csv") {
                        csv_data = field
                            .bytes()
                            .await
                            .change_context(ApiError::FailedToConvertToBytes)?;
                    }
                } else if let Some(name) = field.name() {
                    if name == "json" {
                        json_data = field
                            .bytes()
                            .await
                            .change_context(ApiError::FailedToConvertToBytes)?;
                    }
                }
            }
            (Some(csv_data), Some(json_data))
        } else {
            (None, None)
        };

        let json_data = json_data
            .map(|data| serde_json::from_slice::<SimulateDataRequest>(&data))
            .transpose()
            .change_context(ApiError::FailedToParseJsonInput)?
            .unwrap_or_default();

        Ok(UploadDataResolver {
            upload_data,
            json_data,
            csv_data,
        })
    }
}
