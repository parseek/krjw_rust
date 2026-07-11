use std::cmp::Ordering;

use glam::{Vec2, Vec4};

pub trait HaveID {
    fn get_id(&self) -> u64;
}

#[derive(Clone, Copy, Debug)]
pub struct Sprite2D {
    pub origin_px: Vec2,
    pub size_px: Vec2,
    pub uv_tl_px: Vec2,
    pub uv_size_px: Vec2,
}

#[derive(Clone, Debug)]
pub struct Sprite2DObject<T, U>
where T: HaveID + Clone, U: Clone {
    pub spr: Sprite2D,
    pub color: [f32; 4],
    pub transform: U,
    pub pipeline: T,
    pub layer: f64,
}

impl<T, U> Sprite2DObject<T, U>
where T: HaveID + Clone, U: Clone {
    fn cmp_key(&self, other: &Self) -> Ordering {
        self.layer
            .total_cmp(&other.layer)
            .then(self.pipeline.get_id().cmp(&other.pipeline.get_id()))
    }
}

#[derive(Debug)]
pub struct Sprite2DBuffer<T, U>
where T: HaveID + Clone, U: Clone {
    buf: Vec<Sprite2DObject<T, U>>,
    sorted: Vec<usize>,
    buf_ver: u64,
    sorted_ver: u64,
}

impl<T, U> Default for Sprite2DBuffer<T, U>
where T: HaveID + Clone, U: Clone {
    fn default() -> Self {
        Self {
            buf: Vec::new(),
            sorted: Vec::new(),
            buf_ver: 0,
            sorted_ver: 0,
        }
    }
}

impl<T, U> Sprite2DBuffer<T, U>
where T: HaveID + Clone, U: Clone {
    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(additional);
        self.sorted.reserve(additional);
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn push(&mut self, sprite: &Sprite2DObject<T, U>) {
        self.sorted.push(self.buf.len());
        self.buf.push(sprite.clone());
        self.buf_ver += 1;
    }

    /// Ensure the sorted index is up-to-date.
    pub fn sort(&mut self) {
        if self.buf_ver == self.sorted_ver {
            return;
        }
        self.sorted.sort_by(|a, b| {
            let a = &self.buf[*a];
            let b = &self.buf[*b];
            a.cmp_key(b)
        });
        self.sorted_ver = self.buf_ver;
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.sorted.clear();
        self.buf_ver = 0;
        self.sorted_ver = 0;
    }

    /// Iterate over sorted items.
    ///
    /// `on_pipeline_change` is called *before* the first item and every time
    /// the `pipeline.get_id()` changes between consecutive items.
    /// `on_item` is called for each item in sorted order.
    pub fn for_each_sorted<B, F, G>(&mut self, ex: &mut B, mut on_pipeline_change: F, mut on_item: G)
    where
        F: FnMut(&mut B, &T),
        G: FnMut(&mut B, &Sprite2DObject<T, U>),
    {
        self.sort();

        let mut prev_id: Option<u64> = None;

        for &idx in &self.sorted {
            let obj = &self.buf[idx];

            if prev_id != Some(obj.pipeline.get_id()) {
                on_pipeline_change(ex, &obj.pipeline);
                prev_id = Some(obj.pipeline.get_id());
            }

            on_item(ex, obj);
        }
    }
}