use anyhow::bail;
use clap::Parser as _;
use google_sheets4::{
    Error as GSError, Sheets, api::ValueRange, hyper_rustls, hyper_util, yup_oauth2,
};
use secrecy::ExposeSecret;
use serde_json::json;
use sheets::clap::ClapConfig;
use tracing::debug;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // TODO (c-git): Add context on the errors

    tracing_subscriber::registry()
        .with(fmt::layer().with_span_events(FmtSpan::NEW | FmtSpan::CLOSE))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .init();

    // Install crypto provider
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install AWS-LC provider");

    match loadenv::load() {
        Ok(was_found) => debug!(".env file was found: {was_found}"),
        Err(err_msg) => bail!("failed to load .env file: {err_msg:?}"),
    }

    let clap_config = ClapConfig::parse();
    debug!("ClapConfig: {:?}", clap_config);

    // Get an ApplicationSecret instance by some means. It contains the `client_id`
    // and `client_secret`, among other things.
    let secret = yup_oauth2::ApplicationSecret {
        client_id: clap_config.client_id,
        client_secret: clap_config.client_secret.expose_secret().into(),
        auth_uri: clap_config.auth_uri,
        token_uri: clap_config.token_uri,
        ..Default::default()
    };

    // Instantiate the authenticator. It will choose a suitable authentication flow
    // for you, unless you replace  `None` with the desired Flow. Provide your own
    // `AuthenticatorDelegate` to adjust the way it operates and get feedback about
    // what's going on. You probably want to bring in your own `TokenStorage` to
    // persist tokens and retrieve them from storage.
    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()?
        .https_only()
        .enable_http2()
        .build();

    let executor = hyper_util::rt::TokioExecutor::new();
    let auth = yup_oauth2::InstalledFlowAuthenticator::with_client(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
        yup_oauth2::client::CustomHyperClientBuilder::from(
            hyper_util::client::legacy::Client::builder(executor).build(connector),
        ),
    )
    .build()
    .await?;

    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()?
                .https_or_http()
                .enable_http2()
                .build(),
        );
    let hub = Sheets::new(client, auth);
    // TODO (c-git): Look at CLI and see how to save token to not need to reauthenticate for each request
    // TODO (c-git): Persist token between runs of the program (if possible)

    // Read value from Sheet1!A1
    let result = hub
        .spreadsheets()
        .values_get(&clap_config.spreadsheet_id, "Sheet1!A1")
        .doit()
        .await;
    dbg!(result)?;

    // Update the value
    // TODO (c-git): Update the value

    // Write the value back to the sheet
    // TODO (c-git): Need to update this code to write the value updated above
    let req = ValueRange {
        values: Some(vec![vec![json!("00:15:46")]]),
        ..Default::default()
    };

    // You can configure optional parameters by calling the respective setters at
    // will, and execute the final call using `doit()`.
    let result = hub
        .spreadsheets()
        .values_append(req, &clap_config.spreadsheet_id, "Sheet1!A1")
        .value_input_option("USER_ENTERED")
        .include_values_in_response(false)
        .doit()
        .await;

    match result {
        Err(e) => match e {
            // The Error enum provides details about what exactly happened.
            // You can also just use its `Debug`, `Display` or `Error` traits
            GSError::HttpError(_)
            | GSError::Io(_)
            | GSError::MissingAPIKey
            | GSError::MissingToken(_)
            | GSError::Cancelled
            | GSError::UploadSizeLimitExceeded(_, _)
            | GSError::Failure(_)
            | GSError::BadRequest(_)
            | GSError::FieldClash(_)
            | GSError::JsonDecodeError(_, _) => eprintln!("{}", e),
        },
        Ok(res) => println!("Success: {res:?}"),
    }

    Ok(())
}
