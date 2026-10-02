use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image::{ImageBuffer, Rgba};
use std::fs::File;
use std::path::Path;

fn clamp(val: f32, min: f32, max: f32) -> f32 {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn sd_round_box(px: f32, py: f32, bx: f32, by: f32, r: f32) -> f32 {
    let qx = px.abs() - bx + r;
    let qy = py.abs() - by + r;
    (qx.max(0.0).hypot(qy.max(0.0))) + qx.max(qy).min(0.0) - r
}

fn render_icon(size: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let mut img = ImageBuffer::new(size, size);
    let fsize = size as f32;

    for y in 0..size {
        for x in 0..size {
            let u = (x as f32 + 0.5) / fsize * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / fsize * 2.0 - 1.0;
            let r = u.hypot(v);

            let disc_alpha = smoothstep(0.92, 0.88, r);
            if disc_alpha <= 0.001 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                continue;
            }

            let angle = v.atan2(u);
            let angle_t = (angle.sin() * 0.5 + 0.5).powf(1.2);
            // Radiant Crimson Red outer ring (crimson-rose #E61937 to bright ruby #FF2A55)
            let ring_r = (230.0 * (1.0 - angle_t) + 255.0 * angle_t) as u8;
            let ring_g = (25.0 * (1.0 - angle_t) + 42.0 * angle_t) as u8;
            let ring_b = (55.0 * (1.0 - angle_t) + 85.0 * angle_t) as u8;

            let ring_dist = (r - 0.85).abs();
            let ring_mask = smoothstep(0.07, 0.01, ring_dist);

            // Deep obsidian charcoal background disc
            let bg_t = clamp((v + 0.8) / 1.6, 0.0, 1.0);
            let bg_r = (14.0 * (1.0 - bg_t) + 24.0 * bg_t) as u8;
            let bg_g = (15.0 * (1.0 - bg_t) + 26.0 * bg_t) as u8;
            let bg_b = (22.0 * (1.0 - bg_t) + 36.0 * bg_t) as u8;

            let mut pix_r = (bg_r as f32 * (1.0 - ring_mask) + ring_r as f32 * ring_mask) as u8;
            let mut pix_g = (bg_g as f32 * (1.0 - ring_mask) + ring_g as f32 * ring_mask) as u8;
            let mut pix_b = (bg_b as f32 * (1.0 - ring_mask) + ring_b as f32 * ring_mask) as u8;

            let bars = [
                (-0.42, 0.065, 0.22, 0.05),
                (-0.21, 0.065, 0.44, 0.05),
                (0.00, 0.065, 0.60, 0.05),
                (0.21, 0.065, 0.42, 0.05),
                (0.42, 0.065, 0.24, 0.05),
            ];

            let mut max_bar_mask = 0.0f32;
            let mut bar_grad_t = 0.0f32;

            for &(bx, half_w, half_h, corner_r) in &bars {
                let d = sd_round_box(u - bx, v, half_w, half_h, corner_r);
                let mask = smoothstep(0.03, -0.01, d);
                if mask > max_bar_mask {
                    max_bar_mask = mask;
                    bar_grad_t = clamp((-v + half_h) / (2.0 * half_h), 0.0, 1.0);
                }
            }

            if max_bar_mask > 0.0 {
                // Soundwave equalizer bars: rich crimson red (#DC143C) to bright luminous ruby (#FF4D6D)
                let bar_r = (220.0 * (1.0 - bar_grad_t) + 255.0 * bar_grad_t) as u8;
                let bar_g = (20.0 * (1.0 - bar_grad_t) + 77.0 * bar_grad_t) as u8;
                let bar_b = (55.0 * (1.0 - bar_grad_t) + 109.0 * bar_grad_t) as u8;

                pix_r = (pix_r as f32 * (1.0 - max_bar_mask) + bar_r as f32 * max_bar_mask) as u8;
                pix_g = (pix_g as f32 * (1.0 - max_bar_mask) + bar_g as f32 * max_bar_mask) as u8;
                pix_b = (pix_b as f32 * (1.0 - max_bar_mask) + bar_b as f32 * max_bar_mask) as u8;
            }

            let alpha = (disc_alpha * 255.0).round() as u8;
            img.put_pixel(x, y, Rgba([pix_r, pix_g, pix_b, alpha]));
        }
    }

    img
}

fn main() {
    let icons_dir = Path::new("icons");
    if !icons_dir.exists() {
        std::fs::create_dir_all(icons_dir).expect("Failed to create icons directory");
    }

    let png_specs = [
        (32, "icons/32x32.png"),
        (64, "icons/64x64.png"),
        (128, "icons/128x128.png"),
        (256, "icons/128x128@2x.png"),
        (512, "icons/icon.png"),
    ];

    for &(size, path) in &png_specs {
        let img = render_icon(size);
        img.save(path)
            .unwrap_or_else(|e| panic!("Failed to save {}: {}", path, e));
        println!("Generated {} ({}x{})", path, size, size);
    }

    let mut icon_dir = IconDir::new(ResourceType::Icon);
    let ico_sizes = [16, 24, 32, 48, 64, 128, 256];
    for &size in &ico_sizes {
        let img = render_icon(size);
        let raw = img.into_raw();
        let icon_image = IconImage::from_rgba_data(size, size, raw);
        let entry = IconDirEntry::encode(&icon_image).expect("Failed to encode ico entry");
        icon_dir.add_entry(entry);
    }

    let ico_path = "icons/icon.ico";
    let file = File::create(ico_path).expect("Failed to create icon.ico");
    icon_dir.write(file).expect("Failed to write icon.ico");
    println!("Generated {}", ico_path);
}
