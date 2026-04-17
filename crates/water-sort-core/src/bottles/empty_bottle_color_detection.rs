use lazy_static::lazy_static;
use opencv::{
    core::{Mat, MatTraitConst, Vec3b},
    imgproc,
};

#[cfg(target_os = "windows")]
use opencv::core::AlgorithmHint;

fn hex_to_bgr(hex_color: &str) -> Vec3b {
    let hex_color = hex_color.trim_start_matches('#');
    let r = u8::from_str_radix(&hex_color[0..2], 16).unwrap();
    let g = u8::from_str_radix(&hex_color[2..4], 16).unwrap();
    let b = u8::from_str_radix(&hex_color[4..6], 16).unwrap();
    Vec3b::from([b, g, r])
}

fn bgr_to_hsv(pixel: Vec3b) -> Vec3b {
    let b = pixel[0] as f32 / 255.0;
    let g = pixel[1] as f32 / 255.0;
    let r = pixel[2] as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };

    Vec3b::from([
        (hue / 2.0).round().clamp(0.0, 179.0) as u8,
        (saturation * 255.0).round().clamp(0.0, 255.0) as u8,
        (max * 255.0).round().clamp(0.0, 255.0) as u8,
    ])
}

fn compute_hsv_range(hex_colors: &[&str], padding: (i32, i32, i32)) -> (Vec3b, Vec3b) {
    let hsv_values = hex_colors
        .iter()
        .map(|color| bgr_to_hsv(hex_to_bgr(color)))
        .collect::<Vec<_>>();

    let h_min = hsv_values.iter().map(|v| v[0]).min().unwrap_or(0) as i32 - padding.0;
    let h_max = hsv_values.iter().map(|v| v[0]).max().unwrap_or(0) as i32 + padding.0;
    let s_min = hsv_values.iter().map(|v| v[1]).min().unwrap_or(0) as i32 - padding.1;
    let s_max = hsv_values.iter().map(|v| v[1]).max().unwrap_or(0) as i32 + padding.1;
    let v_min = hsv_values.iter().map(|v| v[2]).min().unwrap_or(0) as i32 - padding.2;
    let v_max = hsv_values.iter().map(|v| v[2]).max().unwrap_or(0) as i32 + padding.2;

    (
        Vec3b::from([
            h_min.clamp(0, 179) as u8,
            s_min.clamp(0, 255) as u8,
            v_min.clamp(0, 255) as u8,
        ]),
        Vec3b::from([
            h_max.clamp(0, 179) as u8,
            s_max.clamp(0, 255) as u8,
            v_max.clamp(0, 255) as u8,
        ]),
    )
}

lazy_static! {
    static ref EMPTY_BOTTLE_RANGE: (Vec3b, Vec3b) = compute_hsv_range(
        &[
            "#683d2b", "#65382c", "#6c3c31", "#62382b", "#64392c", "#63392b", "#62382b", "#64392d",
            "#63382c", "#63382c", "#64392d", "#64392c", "#875643", "#875643",
        ],
        (8, 30, 30),
    );
}

fn cvt_color_bgr_to_hsv(src: &Mat, dst: &mut Mat) -> opencv::Result<()> {
    #[cfg(target_os = "windows")]
    {
        imgproc::cvt_color(
            src,
            dst,
            imgproc::COLOR_BGR2HSV,
            0,
            AlgorithmHint::ALGO_HINT_DEFAULT,
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        imgproc::cvt_color(src, dst, imgproc::COLOR_BGR2HSV, 0)
    }
}

pub fn is_empty_bottle_color(image_bgr: &Mat, threshold: f64) -> opencv::Result<bool> {
    let mut hsv = Mat::default();
    cvt_color_bgr_to_hsv(image_bgr, &mut hsv)?;

    let (lower, upper) = &*EMPTY_BOTTLE_RANGE;
    let mut match_count = 0usize;
    let mut total_count = 0usize;

    for row in 0..hsv.rows() {
        for col in 0..hsv.cols() {
            let pixel = *hsv.at_2d::<Vec3b>(row, col)?;
            total_count += 1;
            if pixel[0] >= lower[0]
                && pixel[0] <= upper[0]
                && pixel[1] >= lower[1]
                && pixel[1] <= upper[1]
                && pixel[2] >= lower[2]
                && pixel[2] <= upper[2]
            {
                match_count += 1;
            }
        }
    }

    if total_count == 0 {
        return Ok(false);
    }

    Ok((match_count as f64) / (total_count as f64) >= threshold)
}
