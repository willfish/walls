use std::time::Duration;

use reqwest::header::HeaderMap;
use reqwest::{Client, Response, StatusCode};

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
    mut build_request: impl FnMut() -> reqwest::RequestBuilder,
) -> anyhow::Result<Response> {
    let mut attempt = 1;
    loop {
        match build_request().send().await {
            Ok(resp) => {
                let status = resp.status();
                if !is_transient_status(status) || attempt == MAX_ATTEMPTS {
                    return Ok(resp.error_for_status()?);
                }
            }
            Err(error) if is_transient_error(&error) && attempt < MAX_ATTEMPTS => {}
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
