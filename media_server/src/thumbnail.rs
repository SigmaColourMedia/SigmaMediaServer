use std::fs;

use webp::PixelLayout;

use thumbnail_image_extractor::ImageData;

pub fn save_thumbnail_to_storage(id: u32, image_data: ImageData) {
    let encoder = webp::Encoder::new(
        &image_data.data_buffer,
        PixelLayout::Rgb,
        image_data.width as u32,
        image_data.height as u32,
    );

    let encoded = encoder.encode(75.0);
    let path = format!("../temp/{}.webp", id);
    if let Err(e) = fs::write(&path, encoded.as_ref()) {
        eprintln!("Error writing thumbnail to folder {}", e)
    }
}
