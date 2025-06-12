use clap::Parser;
use crate::libwebp::ConversionMethod;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Instance URL (e.g. https://grist.mydomain.net/api)
    #[arg(short='u', long, env = "GIO_BASE_URL")]
    pub base_url: String,

    /// Temporary directory (e.g. /tmp/)
    #[arg(short='d', long, env = "GIO_TEMPORARY_DIRECTORY")]
    pub dir: String,

    /// Grist API-token
    #[clap(short='t', long, env = "GIO_API_TOKEN")]
    pub token: String,

    /// Attachment conversion method
    #[clap(short='m', long, default_value_t = ConversionMethod::Normal, env = "GIO_CONVERSION_METHOD")]
    pub conversion_method: ConversionMethod,

    /// A specific document or nothing to scan all documents
    #[clap(short='s', long)]
    pub specific_document: Option<String>,
    
    /// The limit of concurrent attachment downloads 
    #[clap(short='c', long, default_value_t = 5, env = "GIO_CONCURRENT_DOWNLOADS")]
    pub concurrent_downloads: usize,
}