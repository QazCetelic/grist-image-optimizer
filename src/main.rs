mod libwebp;
mod args;

use crate::libwebp::{webp_convert, webp_install_check, ConversionMethod};
use clap::Parser;
use grist_client::apis::attachments_api::{download_attachment, list_attachments, upload_attachments};
use grist_client::apis::columns_api::list_columns;
use grist_client::apis::configuration::Configuration;
use grist_client::apis::orgs_api::list_orgs;
use grist_client::apis::records_api::{list_records, modify_records};
use grist_client::apis::tables_api::list_tables;
use grist_client::apis::workspaces_api::{describe_workspace, list_workspaces};
use grist_client::models;
use grist_client::models::get_fields::Type;
use grist_client::models::{AttachmentMetadataListRecordsInner, RecordsList};
use serde_json::Value;
use serde_json::Value::Array;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use crate::args::Args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if !webp_install_check().await {
        return Err("The cwebp utility is missing".into());
    }
    let dir_metadata = fs::metadata(&args.dir)?;
    if !dir_metadata.is_dir() {
        return Err("The specified directory is not a directory".into());
    }

    let configuration = Configuration::new(args.base_url, Some(args.token));
    optimize_attachments(&configuration, args.conversion_method, &args.dir, &args.specific_document).await;

    Ok(())
}

const WEBP_QUALITY: usize = 25;

async fn optimize_attachments(configuration: &Configuration, conversion_method: ConversionMethod, image_folder: &str, specific_doc: &Option<String>) {
    if let Ok(orgs) = list_orgs(configuration).await {
        for org in orgs {
            if let Some(org_domain) = org.domain {
                if let Ok(workspaces) = list_workspaces(configuration, &org_domain).await {
                    for workspace in workspaces {
                        if let Ok(workspace_info) = describe_workspace(configuration, workspace.id as i32).await {
                            for doc in workspace_info.docs {
                                if let Some(specific_doc) = specific_doc {
                                    if doc.name != *specific_doc {
                                        println!("Skipped doc '{}'...", doc.name);
                                        continue;
                                    }
                                }
                                println!("Optimizing {} document", doc.name);
                                
                                // Old attachment ID -> New attachment ID
                                let mut attachments_map: HashMap<u64, u64> = Default::default();
                                if let Ok(attachments) = list_attachments(configuration, &doc.id, None, None, None, None, None).await {
                                    let all_attachments_length = attachments.records.len();
                                    let filtered_attachments = filter_attachments(attachments.records);
                                    println!("Optimizing {}/{} attachments in {}", filtered_attachments.len(), all_attachments_length, doc.name);
                                    for attachment in filtered_attachments {
                                        let updated_attachment_id = process_attachment(configuration, conversion_method, image_folder, &doc.id, &attachment).await.expect("Failed to process attachment");
                                        attachments_map.insert(attachment.id, updated_attachment_id);
                                    }
                                }
                                swap_attachments(configuration, &doc.id, &attachments_map).await;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn filter_attachments(attachments: Vec<AttachmentMetadataListRecordsInner>) -> Vec<AttachmentMetadataListRecordsInner> {
    let mut to_process: Vec<AttachmentMetadataListRecordsInner> = Default::default();
    let mut optimized_images: HashSet<String> = Default::default();
    // 1. Scan for optimized images 
    for attachment in &attachments {
        let complete_filename = attachment.fields.file_name.clone().expect("Failed to get complete file name");
        if let Some((file_name, file_type)) = complete_filename.rsplit_once(".") {
            let upper_file_type = file_type.to_uppercase();
            if is_optimized_image_type(&upper_file_type) {
                optimized_images.insert(file_name.to_string());
            }
        }
    }
    // 2. Scan for unoptimized images
    for attachment in attachments {
        let complete_filename = attachment.fields.file_name.clone().expect("Failed to get complete file name");
        if let Some((file_name, file_type)) = complete_filename.rsplit_once(".") {
            let upper_file_type = file_type.to_uppercase();
            if is_unoptimized_image_type(&upper_file_type) {
                if optimized_images.contains(file_name) {
                    println!("Skipping unoptimized image {}, it seems to have already been converted...", file_name);
                }
                else {
                    to_process.push(attachment);
                }
            }
        }
    }
    to_process
}

/// Swap the attachment references in cells with the new optimized images
async fn swap_attachments(configuration: &Configuration, doc_id: &str, attachments_map: &HashMap<u64, u64>) {
    let tables = list_tables(configuration, doc_id).await.expect("Failed to list tables");
    for table in tables.tables {
        let attachment_column_ids = scan_for_attachment_columns(configuration, doc_id, &table.id).await;
        let record_list = list_records(configuration, doc_id, &table.id, None, None, None, None, None, None).await.expect("Failed to list records");

        'record_loop: for record in record_list.records {
            let mut modified_record = record.clone();
            
            for attachment_column in &attachment_column_ids {
                let old_attachment_ids: Vec<u64> = get_attachment_ids(record.fields.get(attachment_column.as_str())).expect("Failed to get attachment ids");
                let mut new_attachment_ids: Vec<u64> = Vec::new();
                for old_attachment_id in old_attachment_ids {
                    if let Some(new_attachment_id) = attachments_map.get(&old_attachment_id) {
                        new_attachment_ids.push(*new_attachment_id);
                    }
                    else {
                        // Attachments get added to the map during processing, therefore this attachment has not been altered and can therefore be skipped
                        continue 'record_loop;
                    }
                }
                let cell_value = create_new_cell_value(&new_attachment_ids).expect("Failed to create new cell value");
                modified_record.fields.insert(attachment_column.to_string(), cell_value);
            }

            // Execute changes one at a time in case something goes wrong
            let records_to_modify: Vec<models::RecordsListRecordsInner> = vec![modified_record];
            modify_records(configuration, doc_id, &table.id, RecordsList::new(records_to_modify), None).await.expect("Failed to modify records");
        }
    }
}

#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
fn create_new_cell_value(ids: &Vec<u64>) -> Result<Value, &'static str> {
    let mut values: Vec<Value> = Vec::new();
    values.push(Value::String("L".to_string()));
    for id in ids {
        values.push(Value::from(serde_json::Number::from(*id)));
    }
    let array = Array(values);
    Ok(array)
}

#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
fn get_attachment_ids(column: Option<&Value>) -> Result<Vec<u64>, &'static str> {
    let attachment_cell = column.ok_or("No attachment cell")?;
    let array = if let Array(arr) = attachment_cell { arr } else { Err("Attachment cell is not an array")? };
    let prefix_value = array.first().ok_or("No elements in array")?.as_str().ok_or("Attachment cell is not a string")?;
    if prefix_value != "L" { return Err("Prefix value (L) is missing") }
    let capacity = array.len().saturating_sub(1); // First value is skipped
    let mut ids = Vec::with_capacity(capacity); 
    for values in array.iter().skip(1) {
        let attachment_id = values.as_u64().ok_or("Element is not a positive integer")?;
        ids.push(attachment_id);
    }
    Ok(ids)
}

async fn scan_for_attachment_columns(configuration: &Configuration, doc_id: &str, table_id: &str) -> Vec<String> {
    let mut attachment_column_ids: Vec<String> = Vec::new();
    let columns_list = list_columns(configuration, doc_id, table_id, Some(true)).await.expect("Failed to list columns");
    if let Some(columns) = columns_list.columns {
        for column in columns {
            let col_type = column.fields.expect("Failed to get column fields").col_type.expect("Failed to get column type");
            // Attachments in Any columns are ignored because of efficiency
            if col_type == Type::Attachments {
                let col_id = column.id.expect("Failed to get column id").to_string();
                attachment_column_ids.push(col_id);
            }
        }
    }
    attachment_column_ids
}

#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
async fn process_attachment(configuration: &Configuration, compression_method: ConversionMethod, image_folder: &str, doc_id: &str, attachment: &AttachmentMetadataListRecordsInner) -> Result<u64, &'static str> {
    if let Some(complete_filename) = attachment.fields.file_name.clone() {
        let (file_name, file_type) = complete_filename.rsplit_once(".").ok_or("Failed to parse filename")?;

        let downloaded_file_path = format!("{image_folder}/{file_name}.jpg");
        let downloaded_file_exists = fs::metadata(downloaded_file_path.clone()).is_ok();
        let converted_file_path = format!("{image_folder}/{file_name}.webp");
        let converted_file_exists = fs::metadata(converted_file_path.clone()).is_ok();

        let upper_file_type = file_type.to_ascii_uppercase();
        let is_unoptimized_image_type = upper_file_type == "JPG" || upper_file_type == "JPEG" || upper_file_type == "PNG";
        if is_unoptimized_image_type {
            let old_size_kb = attachment.fields.file_size.ok_or("Failed to get original file size")? / 1024;
            if !downloaded_file_exists && !converted_file_exists {
                let attachment_bytes = download_attachment(configuration, doc_id, attachment.id).await.map_err(|_| "Failed to download attachment")?;
                fs::write(&downloaded_file_path, attachment_bytes).map_err(|_| "Failed to save attachment")?;

                if !converted_file_exists {
                    webp_convert(compression_method, WEBP_QUALITY, &downloaded_file_path, &converted_file_path).await?;
                    fs::remove_file(&downloaded_file_path).map_err(|_| "Failed to remove original file")?;

                    let converted_file_metadata = fs::metadata(&converted_file_path).map_err(|_| "Failed to get metadata of converted file")?;
                    let converted_file_size_kb = converted_file_metadata.len() / 1024;

                    let attachment_paths = vec![PathBuf::from(converted_file_path)];

                    let ids = upload_attachments(configuration, doc_id, attachment_paths).await.map_err(|_| "Failed to upload attachments")?;
                    let new_attachment_id = *(ids.first().ok_or("Failed to get attachment id")?);

                    println!("Optimized '{file_name}' of type {upper_file_type} with size {old_size_kb}KiB and shrunk it to {converted_file_size_kb}KiB.");
                    
                    // There currently is no endpoint to remove the old attachments
                    
                    return Ok(new_attachment_id); // Use new attachment
                }
            }
        }

        Ok(attachment.id) // Keep current attachment
    }
    else {
        Err("Failed to get file name of attachment")
    }
}

/// file type must be uppercase
#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
fn is_unoptimized_image_type(file_type: &str) -> bool {
    file_type == "JPG" || file_type == "JPEG" || file_type == "PNG"
}
/// file type must be uppercase
#[deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
fn is_optimized_image_type(file_type: &str) -> bool {
    file_type == "WEBP"
}