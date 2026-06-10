//! Stubbed-HTTP test: verifies the [`GlwClient`] composes the right URL
//! for both lookup variants and decodes the documented JSON shape.

#![expect(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently the test module"
)]

use pretty_assertions::assert_eq;
use sl_glw::{EventId, GlwClient, GlwEventKey};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Sample JSON body shared by every test. Mirrors the example in
/// `TODO.md` but pared down to the fields needed for parse validation.
const SAMPLE_JSON: &str = r#"{
    "eventId": 6910,
    "eventName": "test cruise",
    "eventKey": "key cruise",
    "directorName": "LaliaCasau Resident",
    "directorKey": "b609826a-b167-41e0-8e67-9fc0e78b97a1",
    "base": {
        "wind": { "dir": 175, "speed": 17, "gusts": 8, "shifts": 5, "period": 90 },
        "waves": {
            "height": 1.5, "speed": 3, "length": 35,
            "heightVar": 5, "lengthVar": 5,
            "effects": { "speed": 1, "steer": 1 }
        },
        "currents": { "speed": 0, "dir": 180, "waterDepth": 0 }
    }
}"#;

#[tokio::test]
async fn fetches_event_by_id() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/glwDataReq.php"))
        .and(query_param("id", "6910"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SAMPLE_JSON, "application/json"))
        .mount(&server)
        .await;

    let base_url = url::Url::parse(&format!("{}/", server.uri()))?;
    let client = GlwClient::with_base_url(base_url)?;
    let event = client.fetch_event_by_id(EventId::new(6910)).await?;

    let event = event.ok_or("server returned None")?;
    assert_eq!(event.event_id.get(), 6910);
    assert_eq!(event.event_name, "test cruise");
    Ok(())
}

#[tokio::test]
async fn fetches_event_by_key() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/glwDataReq.php"))
        .and(query_param("key", "key cruise"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(SAMPLE_JSON, "application/json"))
        .mount(&server)
        .await;

    let base_url = url::Url::parse(&format!("{}/", server.uri()))?;
    let client = GlwClient::with_base_url(base_url)?;
    let event = client
        .fetch_event_by_key(&GlwEventKey::new("key cruise"))
        .await?;

    let event = event.ok_or("server returned None")?;
    assert_eq!(event.event_key.as_str(), "key cruise");
    Ok(())
}

#[tokio::test]
async fn http_404_is_ok_none() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/glwDataReq.php"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let base_url = url::Url::parse(&format!("{}/", server.uri()))?;
    let client = GlwClient::with_base_url(base_url)?;
    let event = client.fetch_event_by_id(EventId::new(0)).await?;
    assert!(event.is_none(), "404 should map to Ok(None)");
    Ok(())
}

#[tokio::test]
async fn http_200_empty_object_is_ok_none() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/glwDataReq.php"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("{}", "application/json"))
        .mount(&server)
        .await;

    let base_url = url::Url::parse(&format!("{}/", server.uri()))?;
    let client = GlwClient::with_base_url(base_url)?;
    let event = client.fetch_event_by_id(EventId::new(0)).await?;
    assert!(event.is_none(), "empty {{}} should map to Ok(None)");
    Ok(())
}
