use glam::Vec2;
use krjw_engine::{Camera2D, ShapeBatch2D};

#[allow(unused)]
/// Build a perspective grid of lines visible within the camera frustum.
pub fn build_grid(sb: &mut ShapeBatch2D, camera: &Camera2D, spacing: Vec2, color: [f32; 4]) {
    let hw = camera.viewport_size.x * 0.5 * camera.zoom.x;
    let hh = camera.viewport_size.y * 0.5 * camera.zoom.y;
    let half_side = (hw * hw + hh * hh).sqrt();
    let cx = camera.position.x;
    let cy = camera.position.y;

    let min_x = ((cx - half_side) / spacing.x).floor() * spacing.x;
    let max_x = ((cx + half_side) / spacing.x).ceil() * spacing.x;
    let min_y = ((cy - half_side) / spacing.y).floor() * spacing.y;
    let max_y = ((cy + half_side) / spacing.y).ceil() * spacing.y;

    let max_lines = 500;
    if ((max_x - min_x) / spacing.x) as usize > max_lines
        || ((max_y - min_y) / spacing.y) as usize > max_lines
    {
        return;
    }

    let shadow = Vec2::new(5.0, 5.0);

    let mut x = min_x;
    while x <= max_x {
        for (off, col) in [(&shadow, [0.0, 0.0, 0.0, 0.2]), (&Vec2::ZERO, color)] {
            sb.add_square_line_no_uv(
                Vec2::new(x, min_y) + *off,
                Vec2::new(x, max_y) + *off,
                10.0,
                col,
            );
        }
        x += spacing.x;
    }

    let mut y = min_y;
    while y <= max_y {
        for (off, col) in [(&shadow, [0.0, 0.0, 0.0, 0.2]), (&Vec2::ZERO, color)] {
            sb.add_square_line_no_uv(
                Vec2::new(min_x, y) + *off,
                Vec2::new(max_x, y) + *off,
                10.0,
                col,
            );
        }
        y += spacing.y;
    }
}
