use glam::Vec2;

use super::transform2d::Transform2D;

/// A collision shape defined in local space.
#[derive(Copy, Clone, Debug)]
pub enum Collider {
    /// Axis-aligned bounding box (rotation is ignored).
    AABB { half_size: Vec2 },
    /// Oriented bounding box (full transform applied).
    Rect { half_size: Vec2 },
    /// Circle (rotation is irrelevant).
    Circle { radius: f32 },
}

/// A collider placed in world (or parent) space via a Transform2D.
pub struct ColliderInstance<'a> {
    pub shape: &'a Collider,
    pub xform: &'a Transform2D,
}

impl<'a> Into<(&'a Collider, &'a Transform2D)> for ColliderInstance<'a> {
    fn into(self) -> (&'a Collider, &'a Transform2D) {
        (self.shape, self.xform)
    }
}

impl<'a> Into<ColliderInstance<'a>> for (&'a Collider, &'a Transform2D) {
    fn into(self) -> ColliderInstance<'a> {
        ColliderInstance::new(self.0, self.1)
    }
}

/// Result of a collision test.
#[derive(Copy, Clone, Debug)]
pub struct Overlap {
    pub hit: bool,
    /// Penetration vector pointing *from* self *toward* other.
    /// Adding this to self's position resolves the overlap.
    pub push: Vec2,
    /// Collision normal (unit vector, from self toward other).
    pub normal: Vec2,
    /// Penetration depth.
    pub depth: f32,
}

impl Overlap {
    fn miss() -> Self {
        Self {
            hit: false,
            push: Vec2::ZERO,
            normal: Vec2::ZERO,
            depth: 0.0,
        }
    }

    fn from_push(push: Vec2) -> Self {
        let depth = push.length();
        if depth < 1e-8 {
            return Self::miss();
        }
        Self {
            hit: true,
            push,
            normal: push / depth,
            depth,
        }
    }
}

impl<'a> ColliderInstance<'a> {
    /// Create a new ColliderInstance with an optional parent transform.
    /// If `parent` is Some, `xform` is treated as local and transformed into
    /// parent space.
    pub fn new(shape: &'a Collider, xform: &'a Transform2D) -> Self {
        Self { shape, xform }
    }

    /// Test whether a world-space point lies inside this collider.
    pub fn contains_point(&self, point: Vec2) -> bool {
        match self.shape {
            Collider::AABB { half_size } => {
                let h = *half_size * self.xform.scale;
                let d = (point - self.xform.pos).abs();
                d.x <= h.x && d.y <= h.y
            }
            Collider::Rect { half_size } => {
                let local = self.xform.inverse_transform_point(point);
                let h = *half_size;
                local.x.abs() <= h.x && local.y.abs() <= h.y
            }
            Collider::Circle { radius } => {
                let r = radius * self.xform.scale.x.max(self.xform.scale.y);
                (point - self.xform.pos).length_squared() <= r * r
            }
        }
    }

    /// Collision detection between two collider instances.
    pub fn overlaps(&self, other: &ColliderInstance) -> Overlap {
        match (self.shape, other.shape) {
            (Collider::AABB { .. }, Collider::AABB { .. }) => aabb_vs_aabb(self, other),
            (Collider::AABB { .. }, Collider::Circle { .. })
            | (Collider::Circle { .. }, Collider::AABB { .. }) => aabb_vs_circle(self, other),
            (Collider::Circle { .. }, Collider::Circle { .. }) => circle_vs_circle(self, other),
            // Any combination involving at least one Rect → SAT
            _ => sat_overlap(self, other),
        }
    }
}

// ─── AABB vs AABB ─────────────────────────────────────────────

fn aabb_vs_aabb(a: &ColliderInstance, b: &ColliderInstance) -> Overlap {
    let Collider::AABB { half_size: ha } = a.shape else {
        unreachable!()
    };
    let Collider::AABB { half_size: hb } = b.shape else {
        unreachable!()
    };

    let ha = *ha * a.xform.scale;
    let hb = *hb * b.xform.scale;
    let pa = a.xform.pos;
    let pb = b.xform.pos;

    let dx = pb.x - pa.x;
    let dy = pb.y - pa.y;
    let ox = ha.x + hb.x - dx.abs();
    let oy = ha.y + hb.y - dy.abs();
    if ox > 0.0 && oy > 0.0 {
        // Push along the axis with the smallest overlap
        if ox < oy {
            let sign = if dx > 0.0 { 1.0 } else { -1.0 };
            Overlap::from_push(Vec2::new(sign * ox, 0.0))
        } else {
            let sign = if dy > 0.0 { 1.0 } else { -1.0 };
            Overlap::from_push(Vec2::new(0.0, sign * oy))
        }
    } else {
        Overlap::miss()
    }
}

// ─── Circle vs Circle ─────────────────────────────────────────

fn circle_vs_circle(a: &ColliderInstance, b: &ColliderInstance) -> Overlap {
    let Collider::Circle { radius: ra } = a.shape else {
        unreachable!()
    };
    let Collider::Circle { radius: rb } = b.shape else {
        unreachable!()
    };

    let ra = ra * a.xform.scale.x.max(a.xform.scale.y);
    let rb = rb * b.xform.scale.x.max(b.xform.scale.y);
    let diff = b.xform.pos - a.xform.pos;
    let dist = diff.length();
    let overlap = ra + rb - dist;
    if overlap > 0.0 {
        let push_dir = if dist > 1e-8 { diff / dist } else { Vec2::X };
        Overlap::from_push(push_dir * overlap)
    } else {
        Overlap::miss()
    }
}

// ─── AABB vs Circle ───────────────────────────────────────────

fn aabb_vs_circle(a: &ColliderInstance, b: &ColliderInstance) -> Overlap {
    // Determine which is AABB and which is Circle
    let (aabb, circle) = match a.shape {
        Collider::AABB { .. } => (a, b),
        _ => (b, a),
    };
    let Collider::AABB { half_size } = aabb.shape else {
        unreachable!()
    };
    let Collider::Circle { radius } = circle.shape else {
        unreachable!()
    };

    let h = *half_size * aabb.xform.scale;
    let r = radius * circle.xform.scale.x.max(circle.xform.scale.y);
    let center = circle.xform.pos;
    let aabb_center = aabb.xform.pos;

    // Closest point on AABB to circle center
    let closest = Vec2::new(
        center.x.clamp(aabb_center.x - h.x, aabb_center.x + h.x),
        center.y.clamp(aabb_center.y - h.y, aabb_center.y + h.y),
    );

    let diff = center - closest;
    let dist = diff.length();
    let overlap = r - dist;
    if overlap > 0.0 {
        let push_dir = if dist > 1e-8 {
            diff / dist
        } else {
            // Circle center is inside AABB → push along nearest edge
            let to_center = center - aabb_center;
            let ox = h.x - to_center.x.abs();
            let oy = h.y - to_center.y.abs();
            if ox < oy {
                Vec2::new(if to_center.x > 0.0 { 1.0 } else { -1.0 }, 0.0)
            } else {
                Vec2::new(0.0, if to_center.y > 0.0 { 1.0 } else { -1.0 })
            }
        };
        Overlap::from_push(push_dir * overlap)
    } else {
        Overlap::miss()
    }
}

// ─── SAT: Separating Axis Theorem ─────────────────────────────
// Handles Rect×Rect, Rect×AABB, Rect×Circle

fn sat_overlap(a: &ColliderInstance, b: &ColliderInstance) -> Overlap {
    // Collect axes to test (up to 4 for two OBBs, 2 for AABB × OBB)
    let axes = match (a.shape, b.shape) {
        (Collider::Rect { .. }, Collider::Rect { .. }) => rect_axes(&a.xform, &b.xform),
        (Collider::Rect { .. }, Collider::AABB { .. })
        | (Collider::AABB { .. }, Collider::Rect { .. }) => rect_axes(&a.xform, &b.xform),
        (Collider::Rect { .. }, Collider::Circle { .. })
        | (Collider::Circle { .. }, Collider::Rect { .. }) => rect_circle_axes(a, b),
        _ => vec![], // shouldn't reach here
    };

    let mut min_overlap = f32::MAX;
    let mut min_axis = Vec2::X;

    for axis in &axes {
        let proj_a = project_on_axis(a, *axis);
        let proj_b = project_on_axis(b, *axis);

        let overlap = interval_overlap(proj_a.0, proj_a.1, proj_b.0, proj_b.1);
        if overlap <= 0.0 {
            return Overlap::miss();
        }
        if overlap < min_overlap {
            min_overlap = overlap;
            min_axis = *axis;
        }
    }

    // Ensure push direction points from a → b
    let delta = b.xform.pos - a.xform.pos;
    if delta.dot(min_axis) < 0.0 {
        min_axis = -min_axis;
    }

    Overlap::from_push(min_axis * min_overlap)
}

/// Collect up to 4 separation axes from two transforms (OBB edge normals).
fn rect_axes(xform_a: &Transform2D, xform_b: &Transform2D) -> Vec<Vec2> {
    let mut axes = Vec::with_capacity(4);
    for xform in [xform_a, xform_b] {
        let (sin, cos) = xform.rot.sin_cos();
        // Two edge normals: (cos, sin) and (-sin, cos)
        let ax = Vec2::new(cos, sin);
        let ay = Vec2::new(-sin, cos);
        if !axes.iter().any(|a: &Vec2| a.dot(ax).abs() > 0.999) {
            axes.push(ax);
        }
        if !axes.iter().any(|a: &Vec2| a.dot(ay).abs() > 0.999) {
            axes.push(ay);
        }
    }
    axes
}

/// Axes for Rect vs Circle: the two rect edge normals + closest corner direction.
fn rect_circle_axes(a: &ColliderInstance, b: &ColliderInstance) -> Vec<Vec2> {
    let (rect_xform, circle_pos) = match a.shape {
        Collider::Rect { .. } => (&a.xform, b.xform.pos),
        _ => (&b.xform, a.xform.pos),
    };

    let (sin, cos) = rect_xform.rot.sin_cos();
    let ax = Vec2::new(cos, sin);
    let ay = Vec2::new(-sin, cos);

    let mut axes = vec![ax, ay];

    // Add closest corner direction
    let local_center = rect_xform.inverse_transform_point(circle_pos);
    let corner = Vec2::new(local_center.x.signum(), local_center.y.signum());
    let corner_world = rect_xform.transform_point(corner);
    let to_corner = (circle_pos - corner_world).normalize_or_zero();
    if to_corner.length_squared() > 0.0 {
        axes.push(to_corner);
    }

    axes
}

/// Project a collider onto an axis, returning (min, max) along that axis.
fn project_on_axis(inst: &ColliderInstance, axis: Vec2) -> (f32, f32) {
    match inst.shape {
        Collider::AABB { half_size } => {
            let h = *half_size * inst.xform.scale;
            let corners = [
                inst.xform.pos + Vec2::new(-h.x, -h.y),
                inst.xform.pos + Vec2::new(h.x, -h.y),
                inst.xform.pos + Vec2::new(-h.x, h.y),
                inst.xform.pos + Vec2::new(h.x, h.y),
            ];
            project_points(&corners, axis)
        }
        Collider::Rect { half_size } => {
            let h = *half_size;
            let local_corners = [
                Vec2::new(-h.x, -h.y),
                Vec2::new(h.x, -h.y),
                Vec2::new(-h.x, h.y),
                Vec2::new(h.x, h.y),
            ];
            let mut world_corners = [Vec2::ZERO; 4];
            for (i, lc) in local_corners.iter().enumerate() {
                world_corners[i] = inst.xform.transform_point(*lc);
            }
            project_points(&world_corners, axis)
        }
        Collider::Circle { radius } => {
            let r = radius * inst.xform.scale.x.max(inst.xform.scale.y);
            let center = inst.xform.pos;
            let proj = center.dot(axis);
            (proj - r, proj + r)
        }
    }
}

fn project_points(points: &[Vec2; 4], axis: Vec2) -> (f32, f32) {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    for p in points {
        let d = p.dot(axis);
        if d < min {
            min = d;
        }
        if d > max {
            max = d;
        }
    }
    (min, max)
}

fn interval_overlap(a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    let o = (a1.min(b1) - a0.max(b0)).max(0.0);
    if o > 0.0 { o } else { -1.0 }
}
