use std::io::{Cursor, Read as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::ImageEncoder as _;

use super::composer::ComposerImage;

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC_BYTES: u64 = 64 * 1024 * 1024;

struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
    exceeded: bool,
}

impl BoundedBytes {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
            exceeded: false,
        }
    }
}

impl std::io::Write for BoundedBytes {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        if buffer.len() > self.limit.saturating_sub(self.bytes.len()) {
            self.exceeded = true;
            return Err(std::io::Error::other(
                "encoded image exceeds the attachment limit",
            ));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum ImagePaste {
    Image(ComposerImage),
    Text(String),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ImagePasteError {
    #[error("Could not access the clipboard: {0}")]
    Clipboard(String),
    #[error("Could not read image {name}: {message}")]
    ReadImage { name: String, message: String },
    #[error("Image {name} exceeds the 20 MiB attachment limit")]
    TooLarge { name: String },
    #[error("Image {name} is not a valid PNG, JPEG, GIF, or WebP file: {message}")]
    InvalidImage { name: String, message: String },
}

pub(crate) fn read_clipboard(cwd: &Path) -> Result<Option<ImagePaste>, ImagePasteError> {
    // Try the system clipboard first; on platforms without direct clipboard
    // access (OHOS, headless, SSH) fall back to an OSC 52 terminal query.
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(error) => {
            #[cfg(unix)]
            if let Some(text) = read_clipboard_text_osc52() {
                if !text.is_empty() {
                    return classify_pasted_text(&text, cwd).map(Some);
                }
                return Ok(None);
            }
            return Err(ImagePasteError::Clipboard(error.to_string()));
        }
    };
    match clipboard.get_image() {
        Ok(image) => {
            return image_from_rgba(
                "clipboard.png",
                image.width,
                image.height,
                image.bytes.as_ref(),
            )
            .map(ImagePaste::Image)
            .map(Some);
        }
        Err(arboard::Error::ContentNotAvailable) => {}
        Err(error) => return Err(ImagePasteError::Clipboard(error.to_string())),
    }
    match clipboard.get_text() {
        Ok(text) if !text.is_empty() => classify_pasted_text(&text, cwd).map(Some),
        Ok(_) | Err(arboard::Error::ContentNotAvailable) => {
            #[cfg(unix)]
            if let Some(text) = read_clipboard_text_osc52() {
                if !text.is_empty() {
                    return classify_pasted_text(&text, cwd).map(Some);
                }
            }
            Ok(None)
        }
        Err(error) => Err(ImagePasteError::Clipboard(error.to_string())),
    }
}

/// Query the terminal clipboard via OSC 52.
///
/// Sends `ESC ] 52 ; c ; ? BEL` and reads the base64-encoded response that the
/// terminal writes back. Used when the system clipboard (`arboard`) is
/// unavailable — e.g. on OHOS where the terminal is the only clipboard
/// provider.
#[cfg(unix)]
fn read_clipboard_text_osc52() -> Option<String> {
    use std::io::{IsTerminal, Read, Write};
    use std::os::fd::AsRawFd;
    use std::time::{Duration, Instant};

    const TIMEOUT: Duration = Duration::from_millis(100);
    const POLL_INTERVAL: Duration = Duration::from_millis(2);
    const READ_CHUNK: usize = 4096;

    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        return None;
    }

    // OSC 52 clipboard query: ESC ] 52 ; c ; ? BEL
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(b"\x1b]52;c;?\x07");
    let _ = stdout.flush();
    drop(stdout);

    let start = Instant::now();
    let mut buf = Vec::with_capacity(1024);

    let fd = std::io::stdin().as_raw_fd();
    // SAFETY: fcntl with F_GETFL/F_SETFL is a standard, thread-safe fd flag
    // manipulation. The original flags are restored before returning so that
    // crossterm's event reader is unaffected.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags < 0 {
            return None;
        }
        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
            return None;
        }

        let mut stdin = std::io::stdin().lock();
        let mut tmp = [0u8; READ_CHUNK];
        while start.elapsed() < TIMEOUT {
            match stdin.read(&mut tmp) {
                Ok(0) => {
                    std::thread::sleep(POLL_INTERVAL);
                }
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                    // Break when we have a complete OSC 52 response
                    // (terminated by BEL or ST).
                    if buf.contains(&b'\x07') || buf.windows(2).any(|w| w == b"\x1b\\") {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(_) => break,
            }
        }

        // Restore original fd flags regardless of the read outcome.
        let _ = libc::fcntl(fd, libc::F_SETFL, flags);
    }

    parse_osc52_response(&buf)
}

/// Parse an OSC 52 clipboard response and return the decoded text.
///
/// Expected format: `ESC ] 52 ; <selector> ; <base64> BEL` (or ST-terminated).
#[cfg(unix)]
fn parse_osc52_response(buf: &[u8]) -> Option<String> {
    use base64::Engine as _;

    let prefix = b"\x1b]52;";
    let start = buf.windows(prefix.len()).position(|w| w == prefix)?;
    let rest = &buf[start + prefix.len()..];

    // Skip the clipboard selector (c, p, s0, ...) and find the data separator.
    let semi = rest.iter().position(|&b| b == b';')?;
    let payload = &rest[semi + 1..];

    // Find the terminator: BEL (\x07) or ST (\x1b\\).
    let end = payload
        .iter()
        .position(|&b| b == b'\x07')
        .or_else(|| payload.windows(2).position(|w| w == b"\x1b\\"))?;

    let base64_data = &payload[..end];
    if base64_data.is_empty() {
        return None;
    }

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .ok()?;
    String::from_utf8(decoded).ok()
}

pub(crate) fn classify_pasted_text(text: &str, cwd: &Path) -> Result<ImagePaste, ImagePasteError> {
    let Some(candidate) = normalize_pasted_path(text, cfg!(windows)) else {
        return Ok(ImagePaste::Text(text.to_string()));
    };
    if !has_supported_image_extension(&candidate) {
        return Ok(ImagePaste::Text(text.to_string()));
    }
    let path = if candidate.is_absolute() {
        candidate
    } else {
        cwd.join(candidate)
    };
    let name = image_name(&path);
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ImagePaste::Text(text.to_string()));
        }
        Err(error) => {
            return Err(ImagePasteError::ReadImage {
                name,
                message: error.to_string(),
            });
        }
    };
    let metadata = match file.metadata() {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return Ok(ImagePaste::Text(text.to_string())),
        Err(error) => {
            return Err(ImagePasteError::ReadImage {
                name,
                message: error.to_string(),
            });
        }
    };
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(ImagePasteError::TooLarge { name });
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ImagePasteError::ReadImage {
            name: name.clone(),
            message: error.to_string(),
        })?;
    image_from_bytes(name, bytes).map(ImagePaste::Image)
}

fn normalize_pasted_path(value: &str, windows: bool) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() || value.contains(['\r', '\n']) {
        return None;
    }
    let raw = value.trim_matches(|character| character == '\'' || character == '"');
    if raw.is_empty() {
        return None;
    }
    if raw.starts_with("file://") {
        return url::Url::parse(raw).ok()?.to_file_path().ok();
    }
    if windows {
        return Some(PathBuf::from(raw));
    }
    let mut unescaped = String::with_capacity(raw.len());
    let mut characters = raw.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if let Some(escaped) = characters.next() {
                unescaped.push(escaped);
            } else {
                unescaped.push(character);
            }
        } else {
            unescaped.push(character);
        }
    }
    Some(PathBuf::from(unescaped))
}

fn has_supported_image_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp"
            )
        })
}

fn image_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("image")
        .to_string()
}

fn image_from_bytes(name: String, bytes: Vec<u8>) -> Result<ComposerImage, ImagePasteError> {
    if bytes.is_empty() {
        return Err(ImagePasteError::InvalidImage {
            name,
            message: "the file is empty".to_string(),
        });
    }
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(ImagePasteError::TooLarge { name });
    }
    let format = image::guess_format(&bytes).map_err(|error| ImagePasteError::InvalidImage {
        name: name.clone(),
        message: error.to_string(),
    })?;
    let mime_type = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        other => {
            return Err(ImagePasteError::InvalidImage {
                name,
                message: format!("unsupported format {other:?}"),
            });
        }
    };
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|error| ImagePasteError::InvalidImage {
            name: name.clone(),
            message: error.to_string(),
        })?;
    Ok(ComposerImage::new(
        uuid::Uuid::new_v4().to_string(),
        name,
        mime_type,
        Arc::from(bytes),
    ))
}

fn image_from_rgba(
    name: &str,
    width: usize,
    height: usize,
    rgba: &[u8],
) -> Result<ComposerImage, ImagePasteError> {
    let name = name.to_string();
    let width = u32::try_from(width).map_err(|error| ImagePasteError::InvalidImage {
        name: name.clone(),
        message: error.to_string(),
    })?;
    let height = u32::try_from(height).map_err(|error| ImagePasteError::InvalidImage {
        name: name.clone(),
        message: error.to_string(),
    })?;
    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || u64::from(width)
            .saturating_mul(u64::from(height))
            .saturating_mul(4)
            > MAX_DECODE_ALLOC_BYTES
    {
        return Err(ImagePasteError::InvalidImage {
            name,
            message: "clipboard image dimensions exceed the attachment limits".to_string(),
        });
    }
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4));
    if expected != Some(rgba.len()) {
        return Err(ImagePasteError::InvalidImage {
            name,
            message: "clipboard pixel buffer has invalid dimensions".to_string(),
        });
    }
    let mut png = BoundedBytes::new(MAX_IMAGE_BYTES as usize);
    let encoded = image::codecs::png::PngEncoder::new(&mut png).write_image(
        rgba,
        width,
        height,
        image::ExtendedColorType::Rgba8,
    );
    if png.exceeded {
        return Err(ImagePasteError::TooLarge { name });
    }
    encoded.map_err(|error| ImagePasteError::InvalidImage {
        name: name.clone(),
        message: error.to_string(),
    })?;
    image_from_bytes(name, png.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, RgbaImage};
    use std::io::Cursor;
    use std::io::Write as _;

    fn png_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255])))
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn pasted_path_normalization_matches_opencode_quotes_urls_and_posix_escapes() {
        assert_eq!(
            normalize_pasted_path("'folder/image.png'", false).unwrap(),
            std::path::PathBuf::from("folder/image.png")
        );
        assert_eq!(
            normalize_pasted_path("folder/my\\ image.png", false).unwrap(),
            std::path::PathBuf::from("folder/my image.png")
        );
        let (file_url, expected) = if cfg!(windows) {
            (
                "file:///C:/tmp/my%20image.png",
                std::path::PathBuf::from(r"C:\tmp\my image.png"),
            )
        } else {
            (
                "file:///tmp/my%20image.png",
                std::path::PathBuf::from("/tmp/my image.png"),
            )
        };
        assert_eq!(
            normalize_pasted_path(file_url, cfg!(windows)).unwrap(),
            expected
        );
    }

    #[test]
    fn valid_file_image_is_snapshotted_with_magic_derived_mime() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.png");
        std::fs::write(&path, png_bytes()).unwrap();

        let paste = classify_pasted_text(path.to_string_lossy().as_ref(), dir.path()).unwrap();
        let ImagePaste::Image(image) = paste else {
            panic!("expected image paste");
        };
        assert_eq!(image.name, "sample.png");
        assert_eq!(image.mime_type, "image/png");
    }

    #[test]
    fn invalid_supported_extension_is_reported_instead_of_attached() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.png");
        std::fs::write(&path, b"not an image").unwrap();

        let error = classify_pasted_text(path.to_string_lossy().as_ref(), dir.path()).unwrap_err();

        assert!(matches!(error, ImagePasteError::InvalidImage { .. }));
    }

    #[test]
    fn oversized_image_is_rejected_before_its_contents_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized.png");
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();

        let error = classify_pasted_text(path.to_string_lossy().as_ref(), dir.path()).unwrap_err();

        assert!(matches!(error, ImagePasteError::TooLarge { .. }));
    }

    #[test]
    fn ordinary_text_and_unsupported_file_paths_remain_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, "hello").unwrap();

        assert!(matches!(
            classify_pasted_text("hello world", dir.path()).unwrap(),
            ImagePaste::Text(text) if text == "hello world"
        ));
        assert!(matches!(
            classify_pasted_text(path.to_string_lossy().as_ref(), dir.path()).unwrap(),
            ImagePaste::Text(_)
        ));
    }

    #[test]
    fn rgba_clipboard_pixels_are_encoded_as_a_valid_png_snapshot() {
        let image = image_from_rgba("clipboard.png", 1, 1, &[10, 20, 30, 255]).unwrap();

        assert_eq!(image.mime_type, "image/png");
        assert_eq!(
            image::guess_format(image.bytes()).unwrap(),
            ImageFormat::Png
        );
    }

    #[test]
    fn clipboard_dimensions_are_rejected_before_png_encoding() {
        let width = MAX_IMAGE_DIMENSION as usize + 1;
        let pixels = vec![0; width * 4];

        let error = image_from_rgba("clipboard.png", width, 1, &pixels).unwrap_err();

        assert!(matches!(
            error,
            ImagePasteError::InvalidImage { message, .. }
                if message == "clipboard image dimensions exceed the attachment limits"
        ));
    }

    #[test]
    fn encoded_clipboard_output_is_bounded_while_the_encoder_is_writing() {
        let mut output = BoundedBytes::new(3);

        let error = output.write_all(&[1, 2, 3, 4]).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert!(output.exceeded);
        assert!(output.bytes.is_empty());
    }

    #[cfg(unix)]
    fn osc52_response(text: &str, terminator: &[u8]) -> Vec<u8> {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
        let mut buf = b"\x1b]52;c;".to_vec();
        buf.extend_from_slice(b64.as_bytes());
        buf.extend_from_slice(terminator);
        buf
    }

    #[cfg(unix)]
    #[test]
    fn osc52_response_with_bel_terminator_decodes_clipboard_text() {
        let buf = osc52_response("hello world", b"\x07");
        assert_eq!(parse_osc52_response(&buf).as_deref(), Some("hello world"));
    }

    #[cfg(unix)]
    #[test]
    fn osc52_response_with_st_terminator_decodes_clipboard_text() {
        let buf = osc52_response("hello world", b"\x1b\\");
        assert_eq!(parse_osc52_response(&buf).as_deref(), Some("hello world"));
    }

    #[cfg(unix)]
    #[test]
    fn osc52_empty_clipboard_returns_none() {
        let buf = b"\x1b]52;c;\x07";
        assert_eq!(parse_osc52_response(buf), None);
    }

    #[cfg(unix)]
    #[test]
    fn osc52_response_with_leading_bytes_still_parses() {
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode("pasted".as_bytes());
        let buf = format!("\x1b[31m\x1b]52;c;{b64}\x07");
        assert_eq!(parse_osc52_response(buf.as_bytes()).as_deref(), Some("pasted"));
    }

    #[cfg(unix)]
    #[test]
    fn osc52_multibyte_utf8_text_decodes_correctly() {
        let buf = osc52_response("你好,世界!🌍", b"\x07");
        assert_eq!(parse_osc52_response(&buf).as_deref(), Some("你好,世界!🌍"));
    }

    #[cfg(unix)]
    #[test]
    fn osc52_non_clipboard_response_returns_none() {
        let buf = b"\x1b]11;rgb:0000/0000/0000\x07";
        assert_eq!(parse_osc52_response(buf), None);
    }

    #[cfg(unix)]
    #[test]
    fn osc52_invalid_base64_returns_none() {
        let buf = b"\x1b]52;c;!!!\x07";
        assert_eq!(parse_osc52_response(buf), None);
    }

    #[cfg(unix)]
    #[test]
    fn osc52_primary_selection_selector_is_accepted() {
        let buf = osc52_response("from selection", b"\x07");
        // Replace 'c' selector with 'p' to simulate a primary-selection response.
        let buf: Vec<u8> = buf
            .iter()
            .enumerate()
            .map(|(i, &b)| if i == 5 { b'p' } else { b })
            .collect();
        assert_eq!(parse_osc52_response(&buf).as_deref(), Some("from selection"));
    }
}
