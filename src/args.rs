use clap::Parser;
use crate::libwebp::ConversionMethod;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Instance URL (e.g. https://grist.mydomain.net/api)
    #[arg(short='u', long)]
    pub base_url: String,

    /// Temporary directory (e.g. /tmp/)
    #[arg(short='d', long)]
    pub dir: String,

    /// Grist API-token
    #[clap(short='t', long)]
    pub token: String,

    /// Attachment conversion method
    #[clap(short='c', long, default_value_t = ConversionMethod::Normal)]
    pub conversion_method: ConversionMethod,

    /// A specific document or nothing to scan all documents
    #[clap(short='s', long)]
    pub specific_document: Option<String>,
}