use anyhow::{Context, bail};
use clap::Parser as _;
use google_sheets4::{
    Error as GSError, Sheets,
    api::ValueRange,
    hyper_rustls,
    hyper_util::{self, client::legacy::connect::HttpConnector},
    yup_oauth2,
};
use hyper_rustls::HttpsConnector;
use secrecy::ExposeSecret;
use serde_json::json;
use sheets::clap::ClapConfig;
use std::fmt::Debug;
use tracing::{Level, debug, info, instrument, warn};
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
};

type Hub = Sheets<HttpsConnector<HttpConnector>>;

trait AsRefStr: AsRef<str> + Debug {}
impl<U: AsRef<str> + Debug> AsRefStr for U {}

trait IntoSerdeJsonValue: Into<serde_json::Value> + Debug {}
impl<U: Into<serde_json::Value> + Debug> IntoSerdeJsonValue for U {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing_and_crypto_provider()?;

    // Load environment and parse environment variables / command line arguments
    match loadenv::load() {
        Ok(was_found) => debug!(".env file was found: {was_found}"),
        Err(err_msg) => bail!("failed to load .env file: {err_msg:?}"),
    }
    let clap_config = ClapConfig::parse();
    debug!("ClapConfig: {:?}", clap_config);

    let hub = get_authorized_sheet_client(&clap_config).await?;

    let cell_value =
        read_a_single_cell_from_sheet(&hub, &clap_config.spreadsheet_id, "Sheet1!A1").await?;

    // Update the value and write to sheet
    write_a_single_cell_to_sheet(
        &hub,
        &clap_config.spreadsheet_id,
        "Sheet1!A1",
        cell_value + 1,
    )
    .await?;

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

#[instrument(skip(hub))]
async fn update_range(
    hub: &Hub,
    spreadsheet_id: &str,
    range: impl AsRefStr,
    values: ValueRange,
) -> anyhow::Result<()> {
    let response = hub
        .spreadsheets()
        .values_update(values, spreadsheet_id, range.as_ref())
        .value_input_option("USER_ENTERED")
        .include_values_in_response(false)
        .doit()
        .await
        .context("failed to update values on sheet")?;
    debug!("Response: {response:?}");
    Ok(())
}

#[instrument(skip(hub))]
async fn write_a_single_cell_to_sheet(
    hub: &Hub,
    spreadsheet_id: &str,
    range: impl AsRefStr,
    value: impl IntoSerdeJsonValue,
) -> anyhow::Result<()> {
    let req = ValueRange {
        values: Some(vec![vec![value.into()]]),
        ..Default::default()
    };
    update_range(hub, spreadsheet_id, range, req).await
}

#[instrument(skip_all)]
async fn get_authorized_sheet_client(clap_config: &ClapConfig) -> anyhow::Result<Hub> {
    // Get an ApplicationSecret instance by some means. It contains the `client_id`
    // and `client_secret`, among other things.
    let secret = yup_oauth2::ApplicationSecret {
        client_id: clap_config.client_id.clone(),
        client_secret: clap_config.client_secret.expose_secret().into(),
        auth_uri: clap_config.auth_uri.clone(),
        token_uri: clap_config.token_uri.clone(),
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
    .persist_tokens_to_disk("google_tokens.json")
    .build()
    .await
    .context("failed to create authenticator")?;

    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build(
            hyper_rustls::HttpsConnectorBuilder::new()
                .with_native_roots()?
                .https_or_http()
                .enable_http2()
                .build(),
        );
    Ok(Sheets::new(client, auth))
}

fn setup_tracing_and_crypto_provider() -> anyhow::Result<()> {
    let result_tracing_subscriber = tracing_subscriber::registry()
        .with(fmt::layer().with_span_events(FmtSpan::NEW | FmtSpan::CLOSE))
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .try_init();
    match result_tracing_subscriber {
        Ok(()) => {}
        Err(err_msg) => eprintln!("failed to setup tracing subscriber: {err_msg:?}"),
    };

    // Install crypto provider
    let did_provider_install_pass = rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .is_ok();
    if !did_provider_install_pass {
        bail!("failed to install AWS-LC provider");
    }

    Ok(())
}

#[instrument(skip(hub), ret)]
async fn read_a_single_cell_from_sheet(
    hub: &Hub,
    spreadsheet_id: &str,
    range: impl AsRefStr,
) -> anyhow::Result<i64> {
    let response = read_range(hub, spreadsheet_id, range).await?;
    info!("Response ValueRange: {response:?}");
    let cell_value = response.values.context("no values found")?;
    let cell_value = cell_value.first().context("major_dimension empty")?;
    let cell_value = cell_value.first().context("minor_dimension empty")?;
    Ok(match json_value_to_i64(cell_value) {
        Ok(x) => x,
        Err(err_msg) => {
            warn!("Returning 0 because of: {err_msg:?}");
            0
        }
    })
}

#[instrument(ret)]
fn json_value_to_i64(value: &serde_json::Value) -> anyhow::Result<i64> {
    Ok(match value {
        serde_json::Value::Number(number) => match number.as_i64() {
            Some(x) => x,
            None => bail!("invalid number format"),
        },
        serde_json::Value::String(x) => match x.parse() {
            Ok(x) => x,
            Err(err_msg) => bail!("failed to parse as i64: {err_msg:?}"),
        },
        _ => bail!("unexpected serde_json::Value variant"),
    })
}

#[instrument(skip(hub), ret(level = Level::DEBUG))]
/// Read value from a range
async fn read_range(
    hub: &Hub,
    spreadsheet_id: &str,
    range: impl AsRefStr,
) -> anyhow::Result<ValueRange> {
    let response = hub
        .spreadsheets()
        .values_get(spreadsheet_id, "Sheet1!A1")
        .doit()
        .await
        .context("failed to get values")?;
    debug!("Response {:?}", response);
    Ok(response.1)
}
