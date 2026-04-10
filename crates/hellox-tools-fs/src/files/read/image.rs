use std::path::Path;

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use hellox_gateway_api::{ContentBlock, ImageSource};

use crate::metadata::ImageInfo;

const MAX_INLINE_BINARY_BYTES: usize = 10_000_000;

pub(super) fn read_image_blocks(
    path: &Path,
    bytes: &[u8],
    image: ImageInfo,
) -> Result<Vec<ContentBlock>> {
    if bytes.len() > MAX_INLINE_BINARY_BYTES {
        return Err(anyhow!(
            "image `{}` is too large to inline ({} bytes > {})",
            path.display(),
            bytes.len(),
            MAX_INLINE_BINARY_BYTES
        ));
    }

    let mut lines = vec![
        format!("file: {}", path.display().to_string().replace('\\', "/")),
        format!("type: {}", image.mime_type),
        format!("size_bytes: {}", bytes.len()),
    ];
    if let (Some(width), Some(height)) = (image.width, image.height) {
        lines.push(format!("dimensions: {}x{}", width, height));
    }

    Ok(vec![
        ContentBlock::Text {
            text: lines.join("\n"),
        },
        ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: image.mime_type.to_string(),
                data: BASE64_STANDARD.encode(bytes),
            },
        },
    ])
}
