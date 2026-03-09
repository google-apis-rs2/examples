use clap::Parser;
use secrecy::SecretString;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about)]
pub struct ClapConfig {
    #[arg(long, env = "CLIENT_ID")]
    pub client_id: String,

    #[arg(long, env = "CLIENT_SECRET")]
    pub client_secret: SecretString,

    #[arg(long, env = "AUTH_URI")]
    pub auth_uri: String,

    #[arg(long, env = "TOKEN_URI")]
    pub token_uri: String,

    #[arg(long, env = "SPREADSHEET_ID")]
    pub spreadsheet_id: String,
}

// TODO (c-git): Add doc strings for struct fields
