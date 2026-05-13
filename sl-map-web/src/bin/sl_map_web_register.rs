//! Local helper that emulates the in-world LSL registration object.
//!
//! POSTs a single registration request to a running `sl-map-web` instance
//! using the configured bearer token and prints the returned set-password
//! URL so you can register / reset an account for testing without being
//! signed into Second Life.

use clap::Parser as _;
use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// CLI options for `sl_map_web_register`.
#[derive(Debug, clap::Parser)]
#[clap(
    name = "sl_map_web_register",
    about = "Emulate the LSL registration object against a local sl-map-web instance.",
    author = clap::crate_authors!(),
    version = clap::crate_version!()
)]
struct Args {
    /// Base URL of the running `sl-map-web` server, e.g.
    /// `http://127.0.0.1:3000`. Trailing slash optional.
    #[clap(long, env = "SL_MAP_WEB_URL")]
    server_url: String,

    /// Pre-shared bearer token that the server expects on
    /// `/api/auth/register`. Must match the server's
    /// `SL_MAP_WEB_LSL_REGISTRATION_BEARER_TOKEN`.
    #[clap(
        long,
        env = "SL_MAP_WEB_LSL_REGISTRATION_BEARER_TOKEN",
        hide_env_values = true,
        value_parser = parse_secret_string,
    )]
    bearer_token: SecretString,

    /// Avatar UUID (the SL "agent key").
    #[clap(long)]
    agent_key: Uuid,

    /// Legacy name, e.g. `"Foo Resident"`.
    #[clap(long)]
    legacy_name: String,

    /// `firstname.lastname` username.
    #[clap(long)]
    username: String,
}

/// Request body sent to `/api/auth/register`. Mirrors
/// `sl_map_web::routes::auth::RegisterRequest`.
#[derive(Debug, Serialize)]
struct RegisterRequest {
    /// avatar UUID.
    agent_key: Uuid,
    /// legacy "Firstname Lastname" form.
    legacy_name: String,
    /// `firstname.lastname` form.
    username: String,
}

/// Subset of the server's response that we care about. Anything else is
/// pretty-printed from the raw JSON.
#[derive(Debug, Deserialize)]
struct RegisterResponse {
    /// the one-time set-password URL.
    set_password_url: String,
    /// when that URL stops being valid.
    expires_at: String,
}

/// Top-level error type for the binary.
#[derive(thiserror::Error, Debug)]
enum Error {
    /// underlying reqwest failure (network, TLS, etc.).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// the server returned a non-2xx response.
    #[error("server returned HTTP {status}: {body}")]
    Status {
        /// HTTP status code from the server.
        status: reqwest::StatusCode,
        /// the response body, verbatim.
        body: String,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

/// clap `value_parser` adaptor that wraps the raw CLI/env string in a
/// [`SecretString`] so `Debug` redaction and zeroize-on-drop apply to the
/// bearer token even in this short-lived helper.
fn parse_secret_string(s: &str) -> Result<SecretString, std::convert::Infallible> {
    Ok(SecretString::from(s.to_owned()))
}

/// Parse the CLI arguments, POST to the registration endpoint, and print
/// the returned set-password URL.
fn run() -> Result<(), Error> {
    let args = Args::parse();
    let url = format!(
        "{}/api/auth/register",
        args.server_url.trim_end_matches('/')
    );
    let body = RegisterRequest {
        agent_key: args.agent_key,
        legacy_name: args.legacy_name,
        username: args.username,
    };
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .bearer_auth(args.bearer_token.expose_secret())
        .json(&body)
        .send()?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        return Err(Error::Status { status, body });
    }
    let parsed: RegisterResponse = response.json()?;
    println!("set-password URL: {}", parsed.set_password_url);
    println!("expires at:       {}", parsed.expires_at);
    Ok(())
}
