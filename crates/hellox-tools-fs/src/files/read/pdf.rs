use std::path::Path;

use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use hellox_gateway_api::{ContentBlock, DocumentSource};
use lopdf::Document;

use crate::metadata::PdfInfo;
use crate::support::truncate_for_output;

pub(super) const DEFAULT_PDF_PAGE_LIMIT: usize = 20;
const MAX_PDF_PAGE_LIMIT: usize = 20;
const MAX_INLINE_PDF_BYTES: usize = 3_000_000;

pub(super) fn read_pdf_blocks(
    path: &Path,
    bytes: &[u8],
    pdf: PdfInfo,
    offset: usize,
    limit: usize,
) -> Result<Vec<ContentBlock>> {
    let limit = usize::min(limit, MAX_PDF_PAGE_LIMIT);
    let offset = usize::max(1, offset);
    let page_count = pdf.page_count;
    let requested_end = offset.saturating_add(limit).saturating_sub(1);
    let end_page = if page_count == 0 {
        requested_end
    } else {
        usize::min(page_count, requested_end)
    };

    let mut lines = vec![
        format!("file: {}", path.display().to_string().replace('\\', "/")),
        "type: application/pdf".to_string(),
        format!("size_bytes: {}", bytes.len()),
        format!("pages: {}", page_count),
        format!("offset: {offset}"),
        format!("limit: {limit}"),
        format!("page_range: {}-{}", offset, end_page),
    ];

    let mut blocks = vec![ContentBlock::Text {
        text: lines.join("\n"),
    }];

    // Best-effort text extraction (valid PDFs only). If it fails, we still provide a base64
    // document block so models that can consume PDFs directly have something to work with.
    if let Ok(doc) = Document::load_mem(bytes) {
        let available_pages = doc.get_pages().len();
        let extracted_end = if available_pages == 0 {
            end_page
        } else {
            usize::min(end_page, available_pages)
        };

        for page in offset..=extracted_end {
            match doc.extract_text(&[page as u32]) {
                Ok(text) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    blocks.push(ContentBlock::Text {
                        text: truncate_for_output(format!("== Page {page} ==\n\n{trimmed}")),
                    });
                }
                Err(_) => break,
            }
        }
    }

    if blocks.len() == 1 {
        if bytes.len() <= MAX_INLINE_PDF_BYTES {
            blocks.push(ContentBlock::Document {
                source: DocumentSource::Base64 {
                    media_type: "application/pdf".to_string(),
                    data: BASE64_STANDARD.encode(bytes),
                },
                title: path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .map(|value| value.to_string()),
                context: None,
                citations: None,
            });
        } else {
            lines.push(format!(
                "note: PDF is too large to inline as base64 ({} bytes > {})",
                bytes.len(),
                MAX_INLINE_PDF_BYTES
            ));
            blocks[0] = ContentBlock::Text {
                text: lines.join("\n"),
            };
        }
    }

    Ok(blocks)
}
