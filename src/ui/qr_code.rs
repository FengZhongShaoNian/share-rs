use std::io::Cursor;
use std::sync::Arc;

use gpui_kit::{Image, ImageFormat};
use image::Luma;
use qrcode::QrCode;

/// Render `value` to a PNG QR code image.
///
/// `None` means the value could not be encoded; callers can render a fallback
/// message instead of crashing the view.
pub fn qr_code_image(value: &str) -> anyhow::Result<Arc<Image>> {
    let code = QrCode::new(value.as_bytes())?;

    let mut buffer = Vec::new();
    code.render::<Luma<u8>>()
        .min_dimensions(320, 320)
        .build()
        .write_to(&mut Cursor::new(&mut buffer), image::ImageFormat::Png)?;

    Ok(Arc::new(Image::from_bytes(ImageFormat::Png, buffer)))
}
