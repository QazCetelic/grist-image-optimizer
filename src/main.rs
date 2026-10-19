mod libwebp;
mod args;

use crate::args::Args;
use crate::libwebp::{webp_convert, webp_install_check, ConversionMethod};
use anyhow::{bail, Context};
use clap::Parser;
use futures::future::join_all;
use grist_client::apis::attachments_api::{download_attachment, list_attachments, upload_attachments};
use grist_client::apis::columns_api::list_columns;
use grist_client::apis::configuration::Configuration;
use grist_client::apis::orgs_api::list_orgs;
use grist_client::apis::records_api::{list_records, modify_records};
use grist_client::apis::tables_api::list_tables;
use grist_client::apis::workspaces_api::{describe_workspace, list_workspaces};
use grist_client::models::get_fields::Type;
use grist_client::models::{AttachmentMetadataListRecordsInner, Doc, RecordsList, RecordsListRecordsInner};
use serde_json::Value;
use std::cmp::max;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use tempfile::{tempdir, NamedTempFile};
use tokio::sync::Semaphore;

// There is no point in converting the file if it's smaller than this
const MIN_SIZE_KB: usize = 100;
const QUALITY_MAX: isize = 70;
const QUALITY_MIN: isize = 3;
const QUALITY_KB_RATIO: usize = 10;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if !webp_install_check().await {
        bail!("The cwebp utility is missing");
    }

    let configuration = Configuration::new(args.base_url, Some(args.token));
    optimize_attachments(&configuration, args.conversion_method, args.concurrent_downloads, args.specific_document.as_deref()).await
}

async fn optimize_attachments(configuration: &Configuration, conversion_method: ConversionMethod, concurrent_downloads: usize, specific_doc: Option<&str>) -> anyhow::Result<()> {
    let Ok(orgs) = list_orgs(configuration).await else { return Ok(()) };
    for org in orgs {
        let Some(org_domain) = org.domain else { continue };
        let Ok(workspaces) = list_workspaces(configuration, &org_domain).await else { continue };
        for workspace in workspaces {
            let Ok(workspace_info) = describe_workspace(configuration, workspace.id as i32).await else { continue };
            for doc in workspace_info.docs {
                if specific_doc.is_some_and(|name| doc.name != name) {
                    println!("Skipped doc '{}'...", doc.name);
                    continue;
                }
                println!("Optimizing {} document", doc.name);
                let attachments_map = optimize_doc_images(configuration, &doc, conversion_method, concurrent_downloads).await?;
                swap_attachments(configuration, &doc.id, &attachments_map).await?;
            }
        }
    }
    Ok(())
}

/// Converts the document's images, returning a map of old attachment ID -> new attachment ID.
async fn optimize_doc_images(configuration: &Configuration, doc: &Doc, conversion_method: ConversionMethod, concurrent_downloads: usize) -> anyhow::Result<HashMap<u64, u64>> {
    let Ok(attachments) = list_attachments(configuration, &doc.id, None, None, None, None, None).await else {
        return Ok(HashMap::new());
    };
    let all_count = attachments.records.len();
    let to_process = filter_attachments(attachments.records)?;
    if to_process.is_empty() {
        return Ok(HashMap::new());
    }
    println!("Optimizing {}/{all_count} attachments in {}", to_process.len(), doc.name);

    // Too many concurrent downloads result in API errors
    let download_semaphore = Semaphore::new(concurrent_downloads);
    let tasks = to_process.iter().map(|attachment| {
        process_attachment(configuration, conversion_method, &doc.id, attachment, &download_semaphore)
    });
    let mut attachments_map = HashMap::new();
    for result in join_all(tasks).await {
        match result {
            Ok(Some((old, new))) => { attachments_map.insert(old, new); }
            Ok(None) => {}
            Err(e) => eprintln!("Failed to process attachment {e}"),
        }
    }
    Ok(attachments_map)
}

fn filter_attachments(attachments: Vec<AttachmentMetadataListRecordsInner>) -> anyhow::Result<Vec<AttachmentMetadataListRecordsInner>> {
    let mut optimized_images = HashSet::new();
    for attachment in &attachments {
        if let Some((file_name, file_type)) = split_filename(attachment)?
            && is_optimized_image_type(&file_type.to_uppercase())
        {
            optimized_images.insert(file_name.to_string());
        }
    }
    let mut to_process = Vec::new();
    for attachment in attachments {
        let Some((file_name, file_type)) = split_filename(&attachment)? else { continue };
        let is_unoptimized = is_unoptimized_image_type(&file_type.to_uppercase());
        let already_converted = optimized_images.contains(file_name);
        if is_unoptimized && already_converted {
            println!("Skipping unoptimized image {file_name}, it seems to have already been converted...");
        } else if is_unoptimized {
            to_process.push(attachment);
        }
    }
    Ok(to_process)
}

fn split_filename(attachment: &AttachmentMetadataListRecordsInner) -> anyhow::Result<Option<(&str, &str)>> {
    let file_name = attachment.fields.file_name.as_deref().context("Failed to get complete file name")?;
    Ok(file_name.rsplit_once('.'))
}

/// Swap the attachment references in cells with the new optimized images
async fn swap_attachments(configuration: &Configuration, doc_id: &str, attachments_map: &HashMap<u64, u64>) -> anyhow::Result<()> {
    let tables = list_tables(configuration, doc_id).await.context("Failed to list tables")?;
    let mut modified_cnt = 0;
    for table in tables.tables {
        modified_cnt += swap_table_attachments(configuration, doc_id, &table.id, attachments_map).await?;
    }
    if modified_cnt > 0 {
        println!("Successfully modified {modified_cnt} records!");
    }
    Ok(())
}

/// Returns the number of records that were updated in the table.
async fn swap_table_attachments(configuration: &Configuration, doc_id: &str, table_id: &str, attachments_map: &HashMap<u64, u64>) -> anyhow::Result<usize> {
    let attachment_column_ids = scan_for_attachment_columns(configuration, doc_id, table_id).await?;
    if attachment_column_ids.is_empty() {
        return Ok(0); // Skip table if there are no columns with the attachment type
    }
    let record_list = list_records(configuration, doc_id, table_id, None, None, None, None, None, None).await.context("Failed to list records")?;
    let mut modified_cnt = 0;
    for record in record_list.records {
        let Some(record) = swap_record_attachments(record, attachments_map, &attachment_column_ids)? else { continue };
        // Execute changes one at a time in case something goes wrong
        modify_records(configuration, doc_id, table_id, RecordsList::new(vec![record]), None).await.context("Failed to modify records")?;
        modified_cnt += 1;
    }
    Ok(modified_cnt)
}

/// Replaces converted attachment IDs in all attachment columns. Returns `None` if nothing changed.
fn swap_record_attachments(mut record: RecordsListRecordsInner, attachments_map: &HashMap<u64, u64>, attachment_column_ids: &[String]) -> anyhow::Result<Option<RecordsListRecordsInner>> {
    let mut changed = false;
    for column_id in attachment_column_ids {
        let old_ids = get_attachment_ids(record.fields.get(column_id))?;
        let (new_ids, column_changed) = swap_ids(&old_ids, attachments_map);
        if column_changed {
            record.fields.insert(column_id.clone(), create_new_cell_value(&new_ids));
            changed = true;
        }
    }
    if !changed {
        return Ok(None);
    }
    // The response includes formula fields that can't be altered, so only send attachment columns
    record.fields.retain(|field, _| attachment_column_ids.contains(field));
    Ok(Some(record))
}

/// Replaces converted IDs and keeps all others (too small, already optimized or failed) as-is.
fn swap_ids(ids: &[u64], attachments_map: &HashMap<u64, u64>) -> (Vec<u64>, bool) {
    let mut changed = false;
    let new_ids = ids.iter().map(|id| match attachments_map.get(id) {
        Some(new_id) => { changed = true; *new_id }
        None => *id,
    });
    (new_ids.collect(), changed)
}

fn create_new_cell_value(ids: &[u64]) -> Value {
    Value::Array(std::iter::once(Value::String("L".to_string()))
        .chain(ids.iter().map(|id| Value::from(*id)))
        .collect())
}

fn get_attachment_ids(column: Option<&Value>) -> anyhow::Result<Vec<u64>> {
    let cell = column.context("No attachment cell")?;
    if cell.is_null() {
        return Ok(Vec::new());
    }
    let array = cell.as_array().context("Attachment cell is not an array")?;
    let prefix = array.first().context("No elements in array")?.as_str().context("Attachment cell is not a string")?;
    if prefix != "L" {
        bail!("Prefix value (L) is missing");
    }
    array.iter().skip(1)
        .map(|value| value.as_u64().context("Element is not a positive integer"))
        .collect()
}

async fn scan_for_attachment_columns(configuration: &Configuration, doc_id: &str, table_id: &str) -> anyhow::Result<Vec<String>> {
    let columns_list = list_columns(configuration, doc_id, table_id, Some(true)).await.context("Failed to list columns")?;
    let mut attachment_column_ids = Vec::new();
    for column in columns_list.columns.unwrap_or_default() {
        let col_type = column.fields.context("Failed to get column fields")?.col_type.context("Failed to get column type")?;
        // Attachments in Any columns are ignored because of efficiency
        if col_type == Type::Attachments {
            attachment_column_ids.push(column.id.context("Failed to get column id")?.to_string());
        }
    }
    Ok(attachment_column_ids)
}

/// Converts the attachment if it is an unoptimized image large enough, returning old and new IDs.
async fn process_attachment(configuration: &Configuration, compression_method: ConversionMethod, doc_id: &str, attachment: &AttachmentMetadataListRecordsInner, download_semaphore: &Semaphore) -> anyhow::Result<Option<(u64, u64)>> {
    let Some((_, file_type)) = split_filename(attachment)? else { return Ok(None) };
    if !is_unoptimized_image_type(&file_type.to_ascii_uppercase()) {
        return Ok(None);
    }
    let size_kb = attachment.fields.file_size.context("Failed to get original file size")? / 1024;
    if size_kb < MIN_SIZE_KB {
        return Ok(None);
    }
    let new = convert_and_upload(configuration, compression_method, doc_id, attachment, download_semaphore).await?;
    Ok(Some((attachment.id, new)))
}

/// Downloads, converts and uploads an attachment, returning the new attachment ID.
async fn convert_and_upload(configuration: &Configuration, compression_method: ConversionMethod, doc_id: &str, attachment: &AttachmentMetadataListRecordsInner, download_semaphore: &Semaphore) -> anyhow::Result<u64> {
    let Some((file_name, file_type)) = split_filename(attachment)? else {
        bail!("Failed to parse filename");
    };
    let upper_file_type = file_type.to_ascii_uppercase();
    let old_size_kb = attachment.fields.file_size.context("Failed to get original file size")? / 1024;

    let download_permit = download_semaphore.acquire().await.context("Failed to acquire download semaphore")?;
    let attachment_bytes = download_attachment(configuration, doc_id, attachment.id).await;
    drop(download_permit);
    let attachment_bytes = attachment_bytes.context("Failed to download attachment")?;

    let mut original_file = NamedTempFile::new()?;
    original_file.write_all(&attachment_bytes).context("Failed to write to temporary file")?;
    let converted_dir = tempdir()?;
    let converted_file = converted_dir.path().join(file_name).with_extension("WEBP");
    let webp_quality = max(QUALITY_MAX - (old_size_kb / QUALITY_KB_RATIO) as isize, QUALITY_MIN) as usize;
    webp_convert(compression_method, webp_quality, &original_file, &converted_file).await.map_err(anyhow::Error::msg)?;

    let converted_size_kb = fs::metadata(&converted_file).context("Failed to get metadata of converted file")?.len() / 1024;
    let ids = upload_attachments(configuration, doc_id, vec![converted_file]).await.context("Failed to upload attachments")?;
    let new_attachment_id = *ids.first().context("Failed to get attachment id")?;
    println!("Optimized '{file_name}' of type {upper_file_type} with size {old_size_kb}KiB and shrunk it to {converted_size_kb}KiB with quality {webp_quality}%.");
    Ok(new_attachment_id)
}

/// file type must be uppercase
fn is_unoptimized_image_type(file_type: &str) -> bool {
    file_type == "JPG" || file_type == "JPEG" || file_type == "PNG"
}
/// file type must be uppercase
fn is_optimized_image_type(file_type: &str) -> bool {
    file_type == "WEBP"
}