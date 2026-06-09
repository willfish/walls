use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::{Client, Response, StatusCode};

use crate::providers::{ProviderRetry, ProviderRetryReason};

pub(crate) const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_ATTEMPTS: u32 = 3;
const RETRY_BACKOFF_BASE_MS: u64 = 100;

pub(crate) fn client() -> anyhow::Result<Client> {
    client_with_timeouts(DEFAULT_REQUEST_TIMEOUT, DEFAULT_CONNECT_TIMEOUT)
}

pub(crate) fn client_with_headers(headers: HeaderMap) -> anyhow::Result<Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .timeout(DEFAULT_REQUEST_TIMEOUT)
        .default_headers(headers)
        .build()?)
}

pub(crate) fn client_with_timeouts(
    request_timeout: Duration,
    connect_timeout: Duration,
) -> anyhow::Result<Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()?)
}

pub(crate) async fn send_with_retries(
    build_request: impl FnMut() -> reqwest::RequestBuilder,
) -> anyhow::Result<Response> {
    send_with_retry_report(build_request)
        .await
        .map(|outcome| outcome.response)
}

pub(crate) struct ProviderHttpOutcome {
    pub(crate) response: Response,
    pub(crate) retries: Vec<ProviderRetry>,
}

pub(crate) async fn send_with_retry_report(
    mut build_request: impl FnMut() -> reqwest::RequestBuilder,
) -> anyhow::Result<ProviderHttpOutcome> {
    let mut attempt = 1;
    let mut retries = Vec::new();
    loop {
        match build_request().send().await {
            Ok(resp) => {
                let status = resp.status();
                if !is_transient_status(status) || attempt == MAX_ATTEMPTS {
                    return Ok(ProviderHttpOutcome {
                        response: resp.error_for_status()?,
                        retries,
                    });
                }
                retries.push(retry_for_status(attempt, status));
            }
            Err(error) if is_transient_error(&error) && attempt < MAX_ATTEMPTS => {
                retries.push(retry_for_error(attempt, &error));
            }
            Err(error) => return Err(error.into()),
        }

        tokio::time::sleep(backoff_delay(attempt)).await;
        attempt += 1;
    }
}

fn is_transient_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn is_transient_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_millis(RETRY_BACKOFF_BASE_MS * u64::from(attempt))
}

fn retry_for_status(attempt: u32, status: StatusCode) -> ProviderRetry {
    let backoff_ms = backoff_ms(attempt);
    if status == StatusCode::TOO_MANY_REQUESTS {
        return ProviderRetry::rate_limited(attempt, backoff_ms);
    }
    ProviderRetry {
        attempt,
        backoff_ms,
        reason: ProviderRetryReason::ServerError,
        status_code: Some(status.as_u16()),
    }
}

fn retry_for_error(attempt: u32, error: &reqwest::Error) -> ProviderRetry {
    let backoff_ms = backoff_ms(attempt);
    let reason = if error.is_timeout() {
        ProviderRetryReason::Timeout
    } else {
        ProviderRetryReason::Connect
    };
    ProviderRetry {
        attempt,
        backoff_ms,
        reason,
        status_code: None,
    }
}

fn backoff_ms(attempt: u32) -> u64 {
    u64::try_from(backoff_delay(attempt).as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn send_with_retries_retries_transient_status() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/flaky"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/flaky"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let client = client().unwrap();
        let response = send_with_retries(|| client.get(format!("{}/flaky", server.uri())))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn send_with_retry_report_records_rate_limit_retry() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/limited"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/limited"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let client = client().unwrap();
        let outcome = send_with_retry_report(|| client.get(format!("{}/limited", server.uri())))
            .await
            .unwrap();

        assert_eq!(outcome.response.status(), StatusCode::OK);
        assert_eq!(outcome.retries.len(), 1);
        assert_eq!(outcome.retries[0].attempt, 1);
        assert_eq!(outcome.retries[0].reason, ProviderRetryReason::RateLimited);
        assert_eq!(outcome.retries[0].status_code, Some(429));
    }

    #[tokio::test]
    async fn send_with_retries_does_not_retry_client_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = client().unwrap();
        let error = send_with_retries(|| client.get(format!("{}/missing", server.uri())))
            .await
            .unwrap_err();

        assert_eq!(
            error
                .downcast_ref::<reqwest::Error>()
                .and_then(reqwest::Error::status),
            Some(StatusCode::NOT_FOUND)
        );
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn bounded_client_applies_request_timeout() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(200)))
            .mount(&server)
            .await;

        let client =
            client_with_timeouts(Duration::from_millis(20), DEFAULT_CONNECT_TIMEOUT).unwrap();
        let error = send_with_retries(|| client.get(format!("{}/slow", server.uri())))
            .await
            .unwrap_err();

        let reqwest_error = error.downcast_ref::<reqwest::Error>().unwrap();
        assert!(reqwest_error.is_timeout());
    }
}
