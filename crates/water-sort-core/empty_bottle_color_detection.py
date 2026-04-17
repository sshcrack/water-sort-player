import cv2
import numpy as np

# Your sample colors in BGR (OpenCV format)
EMPTY_BOTTLE_SAMPLES = [
    "#683d2b",
    "#65382c",
    "#6c3c31",
    "#62382b",
    "#64392c",
    "#63392b",
    "#62382b",
    "#64392d",
    "#63382c",
    "#63382c",
    "#64392d",
    "#64392c",
    "#875643",
    "#875643",
]


def hex_to_hsv(hex_color: str) -> np.ndarray:
    hex_color = hex_color.lstrip("#")
    r, g, b = (int(hex_color[i : i + 2], 16) for i in (0, 2, 4))
    bgr: np.ndarray = np.array([[[b, g, r]]], dtype=np.uint8)
    hsv: np.ndarray = cv2.cvtColor(bgr, cv2.COLOR_BGR2HSV)
    return hsv[0][0]  # [H, S, V]


def compute_hsv_range(hex_colors: list, padding: tuple = (8, 30, 30)) -> tuple:
    """Compute HSV bounds from sample colors with optional padding."""
    hsv_values = np.array([hex_to_hsv(c) for c in hex_colors])

    h_min = max(0, int(hsv_values[:, 0].min()) - padding[0])
    h_max = min(179, int(hsv_values[:, 0].max()) + padding[0])
    s_min = max(0, int(hsv_values[:, 1].min()) - padding[1])
    s_max = min(255, int(hsv_values[:, 1].max()) + padding[1])
    v_min = max(0, int(hsv_values[:, 2].min()) - padding[2])
    v_max = min(255, int(hsv_values[:, 2].max()) + padding[2])

    lower = np.array([h_min, s_min, v_min])
    upper = np.array([h_max, s_max, v_max])
    return lower, upper


# --- Pre-compute once at startup ---
LOWER_EMPTY, UPPER_EMPTY = compute_hsv_range(EMPTY_BOTTLE_SAMPLES)


def is_empty_bottle_color(image_bgr: np.ndarray, threshold: float = 0.3) -> bool:
    """
    Returns True if enough of the image matches the empty bottle color range.
    image_bgr: cropped bottle ROI in BGR format
    threshold: fraction of pixels that must match (0.0 - 1.0)
    """
    hsv = cv2.cvtColor(image_bgr, cv2.COLOR_BGR2HSV)
    mask = cv2.inRange(hsv, LOWER_EMPTY, UPPER_EMPTY)
    match_ratio = np.count_nonzero(mask) / mask.size
    return bool(match_ratio >= threshold)
