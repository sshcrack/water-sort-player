import numpy as np
import cv2 as cv
import json
import sys
import os
from typing import TypedDict
from empty_bottle_color_detection import is_empty_bottle_color


class BoundingBox(TypedDict):
    x: int
    y: int
    w: int
    h: int


class Bottle(TypedDict):
    bounding_box: BoundingBox
    layers: list[str] | None
    unlock_color: str | None


img_bottle_path = (
    sys.argv[1]
    if len(sys.argv) > 1
    else "../water-sort-player/captures/discovery-level-28.png"
)


img = cv.imread(img_bottle_path)
assert img is not None, "file could not be read"

crop_x = 0
crop_y = 143
crop_width = 360
crop_height = 544

cropped_img = img[crop_y : crop_y + crop_height, crop_x : crop_x + crop_width]

cropped_gray = cv.cvtColor(cropped_img, cv.COLOR_BGR2GRAY)
ret, thresh = cv.threshold(cropped_gray, 127, 255, 0)
contours, hierarchy = cv.findContours(thresh, cv.RETR_EXTERNAL, cv.CHAIN_APPROX_SIMPLE)

found_colors: list[tuple[int, int, int]] = []

curtain_bottles = []

resulting_bottles: list[Bottle] = []

for i in range(len(contours)):
    contour = contours[i]
    bound_x, bound_y, bound_w, bound_h = cv.boundingRect(contour)
    bound_area = bound_w * bound_h

    bottle_to_bounding_box_ratio = float(cv.contourArea(contour)) / float(bound_area)
    is_normal_bottle = bottle_to_bounding_box_ratio > 0.9

    print(f"Contour {i}: x={bound_x}, y={bound_y}, w={bound_w}, h={bound_h}")
    cv.putText(
        cropped_img,
        f"{i}",
        (bound_x, bound_y + bound_h // 2),
        cv.FONT_HERSHEY_SIMPLEX,
        0.5,
        (255, 255, 255),
        1,
    )

    # Cropping off the top part of the bottles. Measuring that gives me
    # About 29/146 of the height

    original_img_full_bottle = 146.0

    color_layer_height = int(28 / original_img_full_bottle * bound_h)

    offset_y = int(29.0 / original_img_full_bottle * bound_h)
    bottle_crop_h = 0
    bound_y += offset_y
    bound_h -= offset_y + bottle_crop_h

    if not is_normal_bottle:
        curtain_bottles.append(i)
        continue

    cv.drawContours(cropped_img, contours, i, (0, 255, 0), 1)
    cv.rectangle(
        cropped_img,
        (bound_x, bound_y),
        (bound_x + bound_w, bound_y + bound_h),
        (255, 0, 0),
        2,
    )

    current_offset_y = 0
    layers: list[str] = []
    layer_idx = 0
    while True:
        layer_x = bound_x
        layer_y = bound_y + current_offset_y
        layer_w = bound_w
        layer_h = color_layer_height

        # Crop like 20% from top and bottom
        height_crop = int(0.3 * layer_h)
        width_crop = int(0.2 * layer_w)

        layer_x += width_crop
        layer_w -= 2 * width_crop
        layer_y += height_crop
        layer_h -= 2 * height_crop

        layer_img = cropped_img[
            layer_y : layer_y + layer_h, layer_x : layer_x + layer_w
        ]

        # Crop img by half vertically
        layer_img_hsv = cv.cvtColor(layer_img, cv.COLOR_BGR2HSV)

        min_v = np.min(layer_img_hsv[:, :, 2])
        max_v = np.max(layer_img_hsv[:, :, 2])

        diff_v = max_v - min_v  # simple scalar, easier to reason about
        rect_thickness = 1

        avg_color = cv.mean(layer_img)[:3]

        # Put avg color as circle in the middle of the layer
        circle_x = layer_x + layer_w // 2
        circle_y = layer_y + layer_h // 2

        # Check if any color is close to the avg_color and if so, use that color of the dict
        found_color = None
        for color in found_colors:
            distance = np.linalg.norm(np.array(color) - np.array(avg_color))
            if distance < 30:  # Threshold for color similarity
                found_colors.append(
                    (
                        int(avg_color[0]),
                        int(avg_color[1]),
                        int(avg_color[2]),
                    )
                )
                found_color = color
                break

        if found_color is None:
            found_color = (
                int(avg_color[0]),
                int(avg_color[1]),
                int(avg_color[2]),
            )

            found_colors.append(found_color)

        cv.circle(cropped_img, (circle_x, circle_y), 5, found_color, -1)

        found_color_hex = "#{:02x}{:02x}{:02x}".format(
            found_color[2], found_color[1], found_color[0]
        )

        if is_empty_bottle_color(layer_img):
            layers.append("empty")
            rect_thickness = 10
        elif diff_v > 180:
                    # Mystery color!
                    layers.append(f"mystery,{found_color_hex}")
                    rect_thickness = 3
        else:
            layers.append(found_color_hex)

        cv.rectangle(
            cropped_img,
            (layer_x, layer_y),
            (layer_x + layer_w, layer_y + layer_h),
            (0, 0, 255),
            rect_thickness,
        )

        current_offset_y += color_layer_height
        # If next doesn't fully fit
        if current_offset_y + color_layer_height > bound_h:
            break

    resulting_bottles.append(
        {
            "bounding_box": {
                "x": bound_x,
                "y": bound_y,
                "w": bound_w,
                "h": bound_h,
            },
            "layers": layers,
            "unlock_color": None,
        }
    )


grouped_curtain_bottles: dict[int, list[int]] = {}

for i in curtain_bottles:
    contour = contours[i]
    bound_x, bound_y, bound_w, bound_h = cv.boundingRect(contour)

    center_x = bound_x + bound_w // 2

    closest_center_x = center_x
    for existing_center_x in grouped_curtain_bottles.keys():
        if abs(existing_center_x - center_x) < 10:  # Threshold for grouping
            closest_center_x = existing_center_x
            break

    if closest_center_x not in grouped_curtain_bottles:
        grouped_curtain_bottles[closest_center_x] = []

    grouped_curtain_bottles[closest_center_x].append(i)


def get_flask_body_contour(contour: cv.typing.MatLike):
    # Get all points, flatten to (N, 2)
    pts = contour.reshape(-1, 2)

    # Find the y value where the contour width (max_x - min_x) jumps
    # Group points by y-row (rounded to nearest few pixels)
    from collections import defaultdict

    rows = defaultdict(list)
    for x, y in pts:
        rows[y].append(x)

    sorted_ys = sorted(rows.keys())

    # Calculate width at each y
    widths = [(y, max(rows[y]) - min(rows[y])) for y in sorted_ys]

    # The neck is narrow; find where width significantly increases (flask start)
    max_width = max(w for _, w in widths)
    flask_start_y = None
    for y, w in widths:
        if w > max_width * 0.5:  # adjust threshold as needed
            flask_start_y = y
            break

    # Filter contour to only points below flask_start_y
    flask_pts = pts[pts[:, 1] >= flask_start_y]  # type: ignore

    return flask_pts.reshape(-1, 1, 2), flask_start_y


for center_x, bottle_indices in grouped_curtain_bottles.items():
    bottle_indices.sort(
        key=lambda i: cv.boundingRect(contours[i])[1]
    )  # Sort by y-coordinate
    if len(bottle_indices) != 3:
        print(f"Unexpected number of bottles in curtain group: {len(bottle_indices)}")
        continue

    flask_contour = contours[bottle_indices[1]]
    random_color = (
        np.random.randint(0, 255),
        np.random.randint(0, 255),
        np.random.randint(0, 255),
    )

    # cv.drawContours(cropped_img, [flask_contour], -1, random_color, 2)

    filtered_flask_contour, _ = get_flask_body_contour(flask_contour)
    flask_x, flask_y, flask_w, flask_h = cv.boundingRect(filtered_flask_contour)

    flask_img = cropped_img[flask_y : flask_y + flask_h, flask_x : flask_x + flask_w]

    # Again, I measured this from a image that is 26x45
    reference_flask_w = 26.0
    reference_flask_h = 45.0

    crop_flask_x = int(9 / reference_flask_w * flask_w)
    crop_flask_y = int(14 / reference_flask_h * flask_h)

    crop_flask_w = int(8 / reference_flask_w * flask_w)
    crop_flask_h = int(23 / reference_flask_h * flask_h)

    cropped_flask_img = flask_img[
        crop_flask_y : crop_flask_y + crop_flask_h,
        crop_flask_x : crop_flask_x + crop_flask_w,
    ]

    cv.drawContours(cropped_img, [filtered_flask_contour], -1, (0, 0, 255), 2)

    color_to_use = (0, 0, 0)

    avg_flask_color = cv.mean(cropped_flask_img)[:3]
    # Find the closest color in avg_color_to_random_color
    closest_color = None
    min_distance = float("inf")
    for color in found_colors:
        distance = np.linalg.norm(np.array(color) - np.array(avg_flask_color))
        if distance < min_distance:
            min_distance = distance
            closest_color = color

    if closest_color is not None:
        color_to_use = closest_color

    cv.rectangle(
        cropped_img,
        (flask_x + crop_flask_x, flask_y + crop_flask_y),
        (flask_x + crop_flask_x + crop_flask_w, flask_y + crop_flask_y + crop_flask_h),
        color_to_use,
        1,
    )

    if closest_color is not None:
        closest_color_hex = "#{:02x}{:02x}{:02x}".format(
            int(closest_color[2]), int(closest_color[1]), int(closest_color[0])
        )

        resulting_bottles.append(
            {
                "bounding_box": {
                    "x": flask_x,
                    "y": flask_y,
                    "w": flask_w,
                    "h": flask_h,
                },
                "layers": None,
                "unlock_color": closest_color_hex,
            }
        )


base_img_name = os.path.basename(img_bottle_path).split(".")[0]
cv.imwrite(f"out/{base_img_name}.png", cropped_img)
print(f"Writing to out/{base_img_name}.png")
def do_bounding_boxes_overlap_vertically(box1: BoundingBox, box2: BoundingBox) -> bool:
    box1_top = box1["y"]
    box1_bottom = box1["y"] + box1["h"]
    box2_top = box2["y"]
    box2_bottom = box2["y"] + box2["h"]

    return not (box1_bottom < box2_top or box2_bottom < box1_top)


def sort_bottles_by_coordinates(bottles: list[Bottle]):
    if not bottles:
        return []

    # Build overlap graph: bottles in the same connected component belong to one row.
    adjacency: list[set[int]] = [set() for _ in range(len(bottles))]
    for i in range(len(bottles)):
        for j in range(i + 1, len(bottles)):
            if do_bounding_boxes_overlap_vertically(
                bottles[i]["bounding_box"], bottles[j]["bounding_box"]
            ):
                adjacency[i].add(j)
                adjacency[j].add(i)

    visited = [False] * len(bottles)
    rows: list[list[Bottle]] = []

    for i in range(len(bottles)):
        if visited[i]:
            continue

        stack = [i]
        visited[i] = True
        component_indices: list[int] = []

        while stack:
            current = stack.pop()
            component_indices.append(current)

            for neighbor in adjacency[current]:
                if not visited[neighbor]:
                    visited[neighbor] = True
                    stack.append(neighbor)

        row_bottles = [bottles[idx] for idx in component_indices]
        row_bottles.sort(key=lambda bottle: bottle["bounding_box"]["x"])
        rows.append(row_bottles)

    # Sort rows from top to bottom by their highest (minimum y) bottle.
    rows.sort(key=lambda row: min(bottle["bounding_box"]["y"] for bottle in row))

    sorted_bottles: list[Bottle] = []
    for row in rows:
        sorted_bottles.extend(row)

    return sorted_bottles


resulting_bottles = sort_bottles_by_coordinates(resulting_bottles)
with open("resulting_bottles.json", "w") as f:
    json.dump(resulting_bottles, f)
