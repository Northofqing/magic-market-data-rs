use std::sync::Arc;

use tonic::metadata::MetadataValue;
use tonic::service::Interceptor;
use tonic::{Request, Status};

#[derive(Clone, Debug)]
pub(crate) struct BearerAuth {
    expected: Arc<[u8]>,
}

impl BearerAuth {
    pub(crate) fn new(token: impl AsRef<[u8]>) -> Result<Self, &'static str> {
        let token = token.as_ref();
        if token.is_empty() {
            return Err("authentication token must not be empty");
        }
        if token.iter().any(u8::is_ascii_whitespace) {
            return Err("authentication token must not contain whitespace");
        }
        Ok(Self {
            expected: Arc::from(token),
        })
    }

    fn accepts(&self, metadata: Option<&MetadataValue<tonic::metadata::Ascii>>) -> bool {
        let Some(value) = metadata.and_then(|value| value.to_str().ok()) else {
            return false;
        };
        let Some(token) = value.strip_prefix("Bearer ") else {
            return false;
        };
        constant_time_equal(token.as_bytes(), &self.expected)
    }
}

impl Interceptor for BearerAuth {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        if self.accepts(request.metadata().get("authorization")) {
            Ok(request)
        } else {
            Err(Status::unauthenticated(
                "missing or invalid bearer credential",
            ))
        }
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let difference = left
        .iter()
        .zip(right)
        .fold(0_u8, |current, (left, right)| current | (left ^ right));
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(value: Option<&str>) -> Request<()> {
        let mut request = Request::new(());
        if let Some(value) = value {
            request
                .metadata_mut()
                .insert("authorization", value.parse().unwrap());
        }
        request
    }

    #[test]
    fn bearer_auth_is_exact_and_fail_closed() {
        let mut auth = BearerAuth::new("secret-token").unwrap();
        assert!(auth.call(request(Some("Bearer secret-token"))).is_ok());
        assert_eq!(
            auth.call(request(Some("Bearer other-token")))
                .unwrap_err()
                .code(),
            tonic::Code::Unauthenticated
        );
        assert!(auth.call(request(None)).is_err());
        assert!(BearerAuth::new("").is_err());
    }
}
