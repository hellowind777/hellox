#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PdfInfo {
    pub(crate) page_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImageInfo {
    pub(crate) mime_type: &'static str,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

pub(crate) fn sniff_pdf(bytes: &[u8]) -> Option<PdfInfo> {
    if !bytes.starts_with(b"%PDF-") {
        return None;
    }

    let body = String::from_utf8_lossy(bytes);
    let page_markers = body.matches("/Type /Page").count();
    let pages_tree_markers = body.matches("/Type /Pages").count();
    let page_count = page_markers.saturating_sub(pages_tree_markers);

    Some(PdfInfo { page_count })
}

pub(crate) fn sniff_image(bytes: &[u8]) -> Option<ImageInfo> {
    if let Some((width, height)) = png_dimensions(bytes) {
        return Some(ImageInfo {
            mime_type: "image/png",
            width: Some(width),
            height: Some(height),
        });
    }
    if let Some((width, height)) = gif_dimensions(bytes) {
        return Some(ImageInfo {
            mime_type: "image/gif",
            width: Some(width),
            height: Some(height),
        });
    }
    if let Some((width, height)) = jpeg_dimensions(bytes) {
        return Some(ImageInfo {
            mime_type: "image/jpeg",
            width: Some(width),
            height: Some(height),
        });
    }
    if let Some((width, height)) = webp_dimensions(bytes) {
        return Some(ImageInfo {
            mime_type: "image/webp",
            width: Some(width),
            height: Some(height),
        });
    }

    None
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }

    Some((
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ))
}

fn gif_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return None;
    }

    Some((
        u16::from_le_bytes(bytes[6..8].try_into().ok()?) as u32,
        u16::from_le_bytes(bytes[8..10].try_into().ok()?) as u32,
    ))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }

    let mut index = 2usize;
    while index + 9 < bytes.len() {
        if bytes[index] != 0xFF {
            index += 1;
            continue;
        }

        let marker = bytes[index + 1];
        index += 2;

        if marker == 0xD8 || marker == 0xD9 {
            continue;
        }

        let segment_len =
            u16::from_be_bytes(bytes.get(index..index + 2)?.try_into().ok()?) as usize;
        if segment_len < 2 || index + segment_len > bytes.len() {
            return None;
        }

        if matches!(
            marker,
            0xC0 | 0xC1
                | 0xC2
                | 0xC3
                | 0xC5
                | 0xC6
                | 0xC7
                | 0xC9
                | 0xCA
                | 0xCB
                | 0xCD
                | 0xCE
                | 0xCF
        ) {
            let height =
                u16::from_be_bytes(bytes.get(index + 3..index + 5)?.try_into().ok()?) as u32;
            let width =
                u16::from_be_bytes(bytes.get(index + 5..index + 7)?.try_into().ok()?) as u32;
            return Some((width, height));
        }

        index += segment_len;
    }

    None
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 30 || !bytes.starts_with(b"RIFF") || bytes.get(8..12) != Some(b"WEBP") {
        return None;
    }

    match bytes.get(12..16)? {
        b"VP8X" => {
            let width = 1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]);
            let height = 1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]);
            Some((width, height))
        }
        _ => None,
    }
}

// Notebook helpers moved into the Read tool implementation.
