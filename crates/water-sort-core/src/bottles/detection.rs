use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, anyhow};
use lazy_static::lazy_static;
use opencv::{
    core::{Mat, MatTrait, MatTraitConst, Point, Rect, Scalar, Vec3b, Vector},
    imgproc,
    prelude::VectorToVec,
};

#[cfg(target_os = "windows")]
use opencv::core::AlgorithmHint;

use crate::constants::{BottleColor, color_distance_sq};

use super::Bottle;
use crate::Pos;
use colored::Colorize;

const CROP_X: i32 = 0;
const CROP_Y: i32 = 143;
const CROP_WIDTH: i32 = 360;
const CROP_HEIGHT: i32 = 495;
const FULL_BOTTLE_HEIGHT: f32 = 146.0;
const COLOR_LAYER_HEIGHT_RATIO: f32 = 28.0 / FULL_BOTTLE_HEIGHT;
const OFFSET_Y_RATIO: f32 = 29.0 / FULL_BOTTLE_HEIGHT;
// Increased the threshold beginning from 30 * 30
const COLOR_MATCH_DISTANCE: u32 = 55 * 55;
const MIN_CURTAIN_BOTTLE_AREA: f64 = 490.0;
const ICE_MIN_RATIO: f64 = 0.60;
const ICE_MAX_RATIO: f64 = 0.85;
const ICE_MIN_WIDTH: i32 = 50;
const ICE_MIN_HEIGHT: i32 = 168;
const NORMALIZED_BOTTLE_WIDTH: i32 = 46;
const NORMALIZED_BOTTLE_HEIGHT: i32 = 166;

lazy_static! {
    pub static ref CROP_RECT: Rect = Rect::new(CROP_X, CROP_Y, CROP_WIDTH, CROP_HEIGHT);
}

#[derive(Debug, Clone)]
struct DetectedBottle {
    bottle: Bottle,
    bounds: Rect,
}

pub fn detect_bottles(
    frame_raw: &Mat,
    frame_display: &mut Mat,
    seen_colors: &mut HashSet<BottleColor>,
) -> Result<Vec<Bottle>> {
    detect_bottles_with_seen_colors(frame_raw, frame_display, seen_colors)
}

fn detect_bottles_with_seen_colors(
    frame_raw: &Mat,
    frame_display: &mut Mat,
    seen_colors: &mut HashSet<BottleColor>,
) -> Result<Vec<Bottle>> {
    let cropped = crop_game_board(frame_raw)?;
    let mut cropped_display = cropped.try_clone()?;
    let contour_candidates = find_contours(&cropped)?;

    let mut detected = Vec::new();
    let mut curtain_indices = Vec::new();

    for (index, contour) in contour_candidates.iter().enumerate() {
        let mut bounds = imgproc::bounding_rect(&contour)?;
        let contour_area = imgproc::contour_area(&contour, false)?;
        let bound_area = (bounds.width * bounds.height) as f64;
        let bottle_to_bounding_box_ratio = if bound_area == 0.0 {
            0.0
        } else {
            contour_area / bound_area
        };
        let is_ice_bottle = bottle_to_bounding_box_ratio >= ICE_MIN_RATIO
            && bottle_to_bounding_box_ratio <= ICE_MAX_RATIO
            && bounds.width > ICE_MIN_WIDTH
            && bounds.height > ICE_MIN_HEIGHT;

        if is_ice_bottle {
            bounds.x += (bounds.width - NORMALIZED_BOTTLE_WIDTH) / 2;
            bounds.width = NORMALIZED_BOTTLE_WIDTH;
            bounds.height = NORMALIZED_BOTTLE_HEIGHT;
        }

        let is_normal_bottle = is_ice_bottle || bottle_to_bounding_box_ratio > 0.9;

        imgproc::put_text(
            &mut cropped_display,
            &format!("{}", index),
            Point::new(bounds.x, bounds.y + bounds.height / 2),
            imgproc::FONT_HERSHEY_SIMPLEX,
            0.5,
            Scalar::new(255.0, 255.0, 255.0, 0.0),
            1,
            imgproc::LINE_AA,
            false,
        )?;

        if !is_normal_bottle {
            curtain_indices.push(index);
            continue;
        }

        let detected_bottle = detect_normal_bottle(
            &mut cropped_display,
            &cropped,
            &contour,
            bounds,
            is_ice_bottle,
            seen_colors,
        )?;

        detected.push(detected_bottle);
    }

    detected.extend(detect_curtain_bottles(
        &mut cropped_display,
        &cropped,
        &contour_candidates,
        &curtain_indices,
        seen_colors,
    )?);

    let mut sorted = sort_detected_bottles_by_coordinates(detected);

    for detected_bottle in &mut sorted {
        let hidden_requirement = detected_bottle.bottle.hidden_requirement_state();
        let is_ice_unlock = detected_bottle.bottle.is_fixed_by_ice();
        let fills = detected_bottle.bottle.get_fills();
        let click_position = bottle_click_position(detected_bottle.bounds);
        detected_bottle.bottle = Bottle::from_fills(fills, Some(click_position));
        detected_bottle.bottle.set_fixed_by_ice(is_ice_unlock);
        detected_bottle
            .bottle
            .set_hidden_requirement(hidden_requirement);

        if !detected_bottle.bottle.is_empty() {
            let mut has_seen_color = false;
            for bottle_color in detected_bottle.bottle.get_fills().iter().rev() {
                if bottle_color.is_empty() {
                    if has_seen_color {
                        log::debug!(
                            "Invalid bottle: {}",
                            detected_bottle
                                .bottle
                                .get_fills()
                                .iter()
                                .rev()
                                .map(|e| e.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        anyhow::bail!(
                            "Unexpected empty layer above fill in bottle at position ({}, {})",
                            click_position.0,
                            click_position.1
                        );
                    }
                    continue;
                }

                has_seen_color = true;
            }
        }
    }

    let mut display_crop_area = frame_display.roi_mut(*CROP_RECT)?;
    cropped_display.copy_to(&mut display_crop_area)?;

    Ok(sorted.into_iter().map(|detected| detected.bottle).collect())
}

fn crop_game_board(frame_raw: &Mat) -> Result<Mat> {
    let region = frame_raw.roi(*CROP_RECT)?;
    Ok(region.try_clone()?)
}

fn find_contours(cropped: &Mat) -> Result<Vector<Vector<Point>>> {
    let mut gray = Mat::default();
    cvt_color(cropped, &mut gray, imgproc::COLOR_BGR2GRAY)?;

    let mut thresh = Mat::default();
    imgproc::threshold(&gray, &mut thresh, 127.0, 255.0, imgproc::THRESH_BINARY)?;

    let mut contours = Vector::<Vector<Point>>::new();
    imgproc::find_contours(
        &thresh,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        Point::new(0, 0),
    )?;

    Ok(contours)
}

fn detect_normal_bottle(
    frame_display: &mut Mat,
    cropped: &Mat,
    contour: &Vector<Point>,
    bounds: Rect,
    is_ice_bottle: bool,
    known_colors: &mut HashSet<BottleColor>,
) -> Result<DetectedBottle> {
    let original_bounds = bounds;
    let offset_y = (OFFSET_Y_RATIO * bounds.height as f32).round() as i32;
    let color_layer_height = (COLOR_LAYER_HEIGHT_RATIO * bounds.height as f32).round() as i32;

    let bounds = Rect::new(
        bounds.x,
        bounds.y + offset_y,
        bounds.width,
        bounds.height - offset_y,
    );

    let mut contour_list = Vector::<Vector<Point>>::new();
    contour_list.push(contour.clone());

    imgproc::draw_contours(
        frame_display,
        &contour_list,
        -1,
        Scalar::new(0.0, 255.0, 0.0, 0.0),
        1,
        imgproc::LINE_8,
        &opencv::core::no_array(),
        i32::MAX,
        Point::new(0, 0),
    )?;

    imgproc::rectangle(
        frame_display,
        bounds,
        Scalar::new(255.0, 0.0, 0.0, 0.0),
        2,
        imgproc::LINE_8,
        0,
    )?;

    let mut fills = Vec::new();
    let mut current_offset_y = 0;

    for layer_index in 0..4 {
        let layer_y = bounds.y + current_offset_y;
        let layer_h = color_layer_height;
        if layer_y + layer_h > bounds.y + bounds.height {
            break;
        }

        let mut layer_rect = build_inner_layer_rect(bounds.x, layer_y, bounds.width, layer_h);

        if is_ice_bottle && layer_index == 3 {
            layer_rect.height /= 2;
        }

        let layer_img = crop_submat(cropped, layer_rect)?;
        let layer_color = classify_layer_color_or_add_to_known(&layer_img, known_colors)?;
        fills.push(layer_color);

        let rect_thickness = match layer_color {
            BottleColor::Empty => 10,
            BottleColor::Mystery => 6,
            BottleColor::Fill(_) => 3,
        };

        imgproc::rectangle(
            frame_display,
            layer_rect,
            if rect_thickness == 3 {
                layer_color.to_pixel_value().into()
            } else {
                Scalar::new(0.0, 0.0, 255.0, 0.0)
            },
            rect_thickness,
            imgproc::LINE_8,
            0,
        )?;

        current_offset_y += color_layer_height;
    }

    fills.reverse();

    let mut bottle = Bottle::from_fills(fills, None);
    bottle.set_fixed_by_ice(is_ice_bottle);

    Ok(DetectedBottle {
        bottle,
        bounds: original_bounds,
    })
}

fn detect_curtain_bottles(
    frame_display: &mut Mat,
    cropped: &Mat,
    contours: &Vector<Vector<Point>>,
    curtain_indices: &[usize],
    known_colors: &mut HashSet<BottleColor>,
) -> Result<Vec<DetectedBottle>> {
    let mut grouped: Vec<((i32, i32), Vec<usize>)> = Vec::new();

    for &index in curtain_indices {
        let contour = contours.get(index)?;
        let bounds = imgproc::bounding_rect(&contour)?;
        let center_x = bounds.x + bounds.width / 2;
        let center_y = bounds.y + bounds.height / 2;

        if let Some((_, indices)) = grouped
            .iter_mut()
            .find(|((existing_center_x, existing_center_y), _)| {
                (*existing_center_x - center_x).abs() < 10 && (*existing_center_y - center_y).abs() < 200
            })
        {
            indices.push(index);
        } else {
            grouped.push(((center_x, center_y), vec![index]));
        }
    }

    let mut detected = Vec::new();

    for (_, mut bottle_indices) in grouped {
        bottle_indices.sort_by_key(|index| {
            let contour = contours.get(*index).unwrap();
            imgproc::bounding_rect(&contour)
                .map(|bounds| bounds.y)
                .unwrap_or(0)
        });

        let best_contour_bottle_idx = (0..bottle_indices.len())
            .max_by_key(|idx| {
                let contour = contours.get(bottle_indices[*idx]).unwrap();
                imgproc::contour_area(&contour, false).unwrap_or(0.0) as i32
            })
            .unwrap_or(0);

        let contour_area = imgproc::contour_area(
            &contours.get(bottle_indices[best_contour_bottle_idx])?,
            false,
        )?;
        if contour_area < MIN_CURTAIN_BOTTLE_AREA {
            log::debug!(
                "Skipping contour group with area {} below threshold: {:?}",
                contour_area,
                bottle_indices
            );
            continue;
        }

        if best_contour_bottle_idx != 1 {
            log::debug!(
                "Unexpected contour grouping for curtain bottle: {:?}",
                best_contour_bottle_idx
            );
        }

        let flask_contour = contours.get(bottle_indices[best_contour_bottle_idx])?;
        let (filtered_flask_contour, _) = get_flask_body_contour(&flask_contour)?;
        let flask_bounds = imgproc::bounding_rect(&filtered_flask_contour)?;

        let crop_flask_x = (9.0 / 26.0 * flask_bounds.width as f32).round() as i32;
        let crop_flask_y = (14.0 / 45.0 * flask_bounds.height as f32).round() as i32;
        let crop_flask_w = (8.0 / 26.0 * flask_bounds.width as f32).round() as i32;
        let crop_flask_h = (23.0 / 45.0 * flask_bounds.height as f32).round() as i32;

        let sampled_rect = Rect::new(
            flask_bounds.x + crop_flask_x,
            flask_bounds.y + crop_flask_y,
            crop_flask_w.max(1),
            crop_flask_h.max(1),
        );

        let sampled_img = crop_submat(cropped, sampled_rect)?;
        let avg_flask_color = mean_bgr(&sampled_img)?;
        log::trace!(
            "Countour index: {}, avg color: {:?}",
            bottle_indices[best_contour_bottle_idx],
            avg_flask_color
        );
        let unlock_color =
            nearest_known_color(avg_flask_color, known_colors).unwrap_or_else(|| {
                log::trace!("Could not detect curtain color, falling back to avg...");
                known_colors.insert(BottleColor::Fill((
                    avg_flask_color[0],
                    avg_flask_color[1],
                    avg_flask_color[2],
                )));

                avg_flask_color
            });

        let mut contour_list = Vector::<Vector<Point>>::new();
        contour_list.push(flask_contour.clone());

        imgproc::draw_contours(
            frame_display,
            &contour_list,
            -1,
            Scalar::new(0.0, 0.0, 255.0, 0.0),
            3,
            imgproc::LINE_8,
            &opencv::core::no_array(),
            i32::MAX,
            Point::new(0, 0),
        )?;

        imgproc::rectangle(
            frame_display,
            sampled_rect,
            Scalar::new(
                unlock_color[0] as f64,
                unlock_color[1] as f64,
                unlock_color[2] as f64,
                0.0,
            ),
            1,
            imgproc::LINE_8,
            0,
        )?;

        let mut bottle = Bottle::from_hidden_requirement(bottle_color_from_bgr(unlock_color), None);
        bottle.set_fixed_by_ice(false);
        detected.push(DetectedBottle {
            bottle,
            bounds: flask_bounds,
        });
    }

    Ok(detected)
}

fn get_flask_body_contour(contour: &Vector<Point>) -> Result<(Vector<Point>, i32)> {
    let points = contour.to_vec();
    if points.is_empty() {
        return Err(anyhow!("flask contour has no points"));
    }

    let mut rows: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    for point in &points {
        rows.entry(point.y).or_default().push(point.x);
    }

    let widths = rows
        .iter()
        .map(|(y, xs)| (*y, *xs.iter().max().unwrap() - *xs.iter().min().unwrap()))
        .collect::<Vec<_>>();

    let max_width = widths.iter().map(|(_, width)| *width).max().unwrap_or(0);
    let flask_start_y = widths
        .iter()
        .find(|(_, width)| *width > (max_width as f32 * 0.5) as i32)
        .map(|(y, _)| *y)
        .unwrap_or(points[0].y);

    let mut filtered = Vector::<Point>::new();
    for point in points.into_iter().filter(|point| point.y >= flask_start_y) {
        filtered.push(point);
    }

    Ok((filtered, flask_start_y))
}

fn classify_layer_color_or_add_to_known(
    layer_img: &Mat,
    known_colors: &mut HashSet<BottleColor>,
) -> Result<BottleColor> {
    if super::empty_bottle_color_detection::is_empty_bottle_color(layer_img, 0.3)? {
        return Ok(BottleColor::Empty);
    }

    let hsv = to_hsv(layer_img)?;
    let (min_v, max_v) = value_range(&hsv)?;
    let avg_color = mean_bgr(layer_img)?;

    if max_v.saturating_sub(min_v) > 180 {
        return Ok(BottleColor::Mystery);
    }

    let color = nearest_known_color(avg_color, known_colors).unwrap_or_else(|| {
        known_colors.insert(BottleColor::Fill((
            avg_color[0],
            avg_color[1],
            avg_color[2],
        )));
        avg_color
    });

    Ok(bottle_color_from_bgr(color))
}

fn nearest_known_color(avg_color: Vec3b, known_colors: &HashSet<BottleColor>) -> Option<Vec3b> {
    let mut closest = None;
    let mut min_distance = u32::MAX;

    let hex_avg = format!(
        "#{:02X}{:02X}{:02X}",
        avg_color[2], avg_color[1], avg_color[0]
    )
    .on_truecolor(avg_color[2], avg_color[1], avg_color[0]);
    log::trace!("--");
    log::trace!("Testing avg color {hex_avg}...");
    for color in known_colors.iter().copied() {
        let color = match color {
            BottleColor::Fill((b, g, r)) => Vec3b::from([b, g, r]),
            _ => continue,
        };

        let distance = color_distance_sq(&avg_color, &color);
        let hex_color = format!("#{:02X}{:02X}{:02X}", color[2], color[1], color[0])
            .on_truecolor(color[2], color[1], color[0]);
        log::trace!("known color: {}, distance: {}", hex_color, distance);
        if distance < min_distance && distance < COLOR_MATCH_DISTANCE {
            min_distance = distance;
            closest = Some(color);
        }
    }

    if let Some(c) = closest.as_ref() {
        let closest_hex =
            format!("#{:02X}{:02X}{:02X}", c[2], c[1], c[0]).on_truecolor(c[2], c[1], c[0]);
        log::trace!("Closest hex: {closest_hex}");
    } else {
        log::trace!("No closest color.");
    }
    log::trace!("--");
    closest
}

fn mean_bgr(image: &Mat) -> Result<Vec3b> {
    let mean = opencv::core::mean(image, &opencv::core::no_array())?;
    Ok(Vec3b::from([
        mean[0].round().clamp(0.0, 255.0) as u8,
        mean[1].round().clamp(0.0, 255.0) as u8,
        mean[2].round().clamp(0.0, 255.0) as u8,
    ]))
}

fn to_hsv(image: &Mat) -> Result<Mat> {
    let mut hsv = Mat::default();
    cvt_color(image, &mut hsv, imgproc::COLOR_BGR2HSV)?;
    Ok(hsv)
}

fn value_range(hsv: &Mat) -> Result<(u8, u8)> {
    let mut min_v = 255u8;
    let mut max_v = 0u8;

    for row in 0..hsv.rows() {
        for col in 0..hsv.cols() {
            let pixel = *hsv.at_2d::<Vec3b>(row, col)?;
            min_v = min_v.min(pixel[2]);
            max_v = max_v.max(pixel[2]);
        }
    }

    Ok((min_v, max_v))
}

fn crop_submat(image: &Mat, rect: Rect) -> Result<Mat> {
    let x = rect.x.max(0).min(image.cols().saturating_sub(1));
    let y = rect.y.max(0).min(image.rows().saturating_sub(1));
    let max_width = image.cols().saturating_sub(x);
    let max_height = image.rows().saturating_sub(y);
    let width = rect.width.max(1).min(max_width);
    let height = rect.height.max(1).min(max_height);
    let roi = image.roi(Rect::new(x, y, width, height))?;
    Ok(roi.try_clone()?)
}

fn build_inner_layer_rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    let height_crop = ((0.3f32 * h as f32).round() as i32).clamp(0, h.saturating_div(2));
    let width_crop = ((0.2f32 * w as f32).round() as i32).clamp(0, w.saturating_div(2));

    Rect::new(
        x + width_crop,
        y + height_crop,
        (w - 2 * width_crop).max(1),
        (h - 2 * height_crop).max(1),
    )
}

fn sort_detected_bottles_by_coordinates(bottles: Vec<DetectedBottle>) -> Vec<DetectedBottle> {
    if bottles.is_empty() {
        return bottles;
    }

    let mut adjacency = vec![Vec::<usize>::new(); bottles.len()];
    for i in 0..bottles.len() {
        for j in (i + 1)..bottles.len() {
            if boxes_overlap_vertically(bottles[i].bounds, bottles[j].bounds) {
                adjacency[i].push(j);
                adjacency[j].push(i);
            }
        }
    }

    let mut visited = vec![false; bottles.len()];
    let mut rows = Vec::new();

    for index in 0..bottles.len() {
        if visited[index] {
            continue;
        }

        let mut stack = vec![index];
        visited[index] = true;
        let mut component = Vec::new();

        while let Some(current) = stack.pop() {
            component.push(current);
            for neighbor in adjacency[current].iter().copied() {
                if !visited[neighbor] {
                    visited[neighbor] = true;
                    stack.push(neighbor);
                }
            }
        }

        component.sort_by_key(|idx| bottles[*idx].bounds.x);
        let min_y = component
            .iter()
            .map(|idx| bottles[*idx].bounds.y)
            .min()
            .unwrap_or_default();
        rows.push((min_y, component));
    }

    rows.sort_by_key(|(min_y, _)| *min_y);

    let mut sorted = Vec::with_capacity(bottles.len());
    for (_, row) in rows {
        for index in row {
            sorted.push(bottles[index].clone());
        }
    }

    sorted
}

fn boxes_overlap_vertically(left: Rect, right: Rect) -> bool {
    let left_top = left.y;
    let left_bottom = left.y + left.height;
    let right_top = right.y;
    let right_bottom = right.y + right.height;

    !(left_bottom < right_top || right_bottom < left_top)
}

fn bottle_color_from_bgr(pixel: Vec3b) -> BottleColor {
    BottleColor::Fill((pixel[0], pixel[1], pixel[2]))
}

fn cvt_color(src: &Mat, dst: &mut Mat, code: i32) -> opencv::Result<()> {
    #[cfg(target_os = "windows")]
    {
        imgproc::cvt_color(src, dst, code, 0, AlgorithmHint::ALGO_HINT_DEFAULT)
    }

    #[cfg(not(target_os = "windows"))]
    {
        imgproc::cvt_color(src, dst, code, 0)
    }
}

fn bottle_click_position(bounds: Rect) -> Pos {
    Pos(
        CROP_X + bounds.x + bounds.width / 2,
        CROP_Y + bounds.y + bounds.height / 2,
    )
}
