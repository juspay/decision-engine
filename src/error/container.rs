use std::error::Error;

use axum::http;

pub struct ContainerError<E> {
    pub(crate) error: error_stack::Report<E>,
    pub(crate) status_code: Option<http::StatusCode>,
    pub(crate) payload: Option<Box<dyn std::any::Any + Send + Sync>>,
}

impl<E: Sync + Send + 'static> ContainerError<E> {
    pub fn get_inner(&self) -> &E {
        self.error.current_context()
    }
}

impl<T> std::fmt::Debug for ContainerError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <error_stack::Report<T> as std::fmt::Debug>::fmt(&self.error, f)
    }
}

impl<T> std::fmt::Display for ContainerError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <error_stack::Report<T> as std::fmt::Display>::fmt(&self.error, f)
    }
}

impl<T: Error + Send + Sync + 'static> Error for ContainerError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Error::source(self.error.current_context())
    }
}

pub trait ErrorTransform<From> {}

impl<T, U> From<ContainerError<T>> for ContainerError<U>
where
    T: error_stack::Context,
    U: error_stack::Context,
    for<'a> U: From<&'a T> + Sync + Send,
    Self: ErrorTransform<ContainerError<T>>,
{
    #[track_caller]
    fn from(value: ContainerError<T>) -> Self {
        let new_error = value.error.current_context().into();
        Self {
            error: value.error.change_context(new_error),
            status_code: None,
            payload: None,
        }
    }
}

impl<T> From<T> for ContainerError<T>
where
    error_stack::Report<T>: From<T>,
{
    #[track_caller]
    fn from(value: T) -> Self {
        Self {
            error: error_stack::Report::from(value),
            status_code: None,
            payload: None,
        }
    }
}

impl<T> From<error_stack::Report<T>> for ContainerError<T> {
    fn from(error: error_stack::Report<T>) -> Self {
        Self {
            error,
            status_code: None,
            payload: None,
        }
    }
}

///
/// # Restricted
///
/// The use of this trait is discouraged, this is only used when low level errors surface, and need
/// to be containerized
///
pub trait ResultContainerExt<T, E> {
    fn change_error(self, error: E) -> Result<T, ContainerError<E>>;
}

impl<T, E1, E2> ResultContainerExt<T, E2> for Result<T, E1>
where
    E1: error_stack::Context,
    E2: error_stack::Context,
{
    #[track_caller]
    fn change_error(self, error: E2) -> Result<T, ContainerError<E2>> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(ContainerError {
                error: error_stack::Report::from(err).change_context(error),
                status_code: None,
                payload: None,
            }),
        }
    }
}

macro_rules! error_transform {
    ($a:ty => $b:ty) => {
        impl crate::error::container::ErrorTransform<crate::error::container::ContainerError<$a>>
            for crate::error::container::ContainerError<$b>
        {
        }
    };
}

impl<E: Sync + Send + 'static + std::error::Error + error_stack::Context> ContainerError<E> {
    pub fn status_code(&self) -> http::StatusCode {
        self.status_code.unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn new_with_status_code_and_payload<T: 'static + Send + Sync>(
        error: E,
        status: http::StatusCode,
        payload: T,
    ) -> Self {
        Self {
            error: error_stack::Report::new(error),
            status_code: Some(status),
            payload: Some(Box::new(payload)),
        }
    }

    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.payload
            .as_ref()
            .and_then(|p| p.downcast_ref::<T>())
    }
}
