//! # Sprite2D — sprite descriptors & pipeline-sorted buffer
//!
//! Core types for describing 2D sprites and batching them by pipeline (texture/shader).
//! 描述 2D 精灵的核心类型，以及按流水线（纹理/着色器）分组的缓冲区。
//!
//! ## Pipeline-sorted iteration / 流水线排序迭代
//!
//! `Sprite2DBuffer::for_each_sorted` automatically:
//! 1. Sorts by `layer` then `pipeline.get_id()` / 先按 layer 排序，再按 pipeline id 排序
//! 2. Calls `on_pipeline_change` before the first item and every time pipeline changes / 首次及 pipeline 切换时调用
//! 3. Calls `on_item` for each sprite / 每个精灵调用一次 on_item

use std::cmp::Ordering;

use glam::Vec2;

/// Trait for types that expose a unique 64-bit identifier.
/// 为类型提供唯一 64 位标识符的 trait。
///
/// Used by `Sprite2DBuffer` to detect pipeline changes during sorted iteration.
/// `Sprite2DBuffer` 用它来在排序迭代中检测流水线切换。
pub trait HaveID {
    /// Returns a unique identifier for this instance.
    /// 返回此实例的唯一标识符。
    fn get_id(&self) -> u64;
}

/// A 2D sprite descriptor — defines a sub-rectangle of a texture (UV) and an origin point.
/// 2D 精灵描述符——定义纹理的子矩形（UV）和原点。
///
/// All values are in **pixel** units (not normalized UV).
/// 所有值均以**像素**为单位（非归一化 UV）。
///
/// # Fields / 字段
///
/// * `origin_px` — origin/pivot point in pixels (e.g. center: `size_px * 0.5`) / 原点/轴点（像素）
/// * `size_px` — rendered size in pixels / 渲染尺寸（像素）
/// * `uv_tl_px` — top-left UV coordinate in pixels into the source texture / 源纹理左上角 UV（像素）
/// * `uv_size_px` — UV rectangle size in pixels / UV 矩形尺寸（像素）
#[derive(Clone, Copy, Debug)]
pub struct Sprite2D {
    pub origin_px: Vec2,
    pub size_px: Vec2,
    pub uv_tl_px: Vec2,
    pub uv_size_px: Vec2,
}

/// A fully-typed sprite object stored in `Sprite2DBuffer`.
/// 存储在 `Sprite2DBuffer` 中的完整类型化精灵对象。
///
/// * `T` — pipeline type (must implement `HaveID` + `Clone`) / 流水线类型
/// * `U` — transform type (any `Clone`) / 变换类型
#[derive(Clone, Debug)]
pub struct Sprite2DObject<T, U>
where T: HaveID + Clone, U: Clone {
    /// Sprite geometry descriptor (UV rect, origin). / 精灵几何描述符（UV 矩形、原点）
    pub spr: Sprite2D,
    /// RGBA colour (pre-multiplied alpha). / RGBA 颜色（预乘 alpha）
    pub color: [f32; 4],
    /// Per-sprite transform (position, scale, rotation). / 每个精灵的变换（位置、缩放、旋转）
    pub transform: U,
    /// Pipeline reference (e.g. `Arc<TextureInfo>`). / 流水线引用
    pub pipeline: T,
    /// Sort layer — lower values are drawn first. / 排序层级——值越小越先绘制
    pub layer: f64,
}

impl<T, U> Sprite2DObject<T, U>
where T: HaveID + Clone, U: Clone {
    /// Compare two objects for sorting: first by `layer`, then by `pipeline.get_id()`.
    /// 比较两个对象的排序顺序：先比 `layer`，再比 `pipeline.get_id()`。
    fn cmp_key(&self, other: &Self) -> Ordering {
        self.layer
            .total_cmp(&other.layer)
            .then(self.pipeline.get_id().cmp(&other.pipeline.get_id()))
    }
}

/// A buffer of sprite objects with a parallel sorted-index array.
/// 精灵对象缓冲区，附带并行的排序索引数组。
///
/// Sprites are inserted in O(1) via `push()`. Sorting is deferred until iteration.
/// `push()` 以 O(1) 插入，排序推迟到迭代时进行。
#[derive(Debug)]
pub struct Sprite2DBuffer<T, U>
where T: HaveID + Clone, U: Clone {
    /// Primary storage — all sprite objects. / 主存储——所有精灵对象
    buf: Vec<Sprite2DObject<T, U>>,
    /// Indices into `buf`, kept in sorted order after `sort()`.
    /// `buf` 的索引数组，`sort()` 后保持排序顺序。
    sorted: Vec<usize>,
    /// Incremented on every structural change (`push`, `clear`).
    /// 每次结构变更（push、clear）时递增。
    buf_ver: u64,
    /// Version when `sorted` was last sorted. When `buf_ver == sorted_ver`, sorting is skipped.
    /// `sorted` 上次排序时的版本号。相等时跳过排序。
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
    /// Reserve capacity for `additional` sprites to avoid re-allocation.
    /// 预分配 `additional` 个精灵的容量，避免重复分配。
    pub fn reserve(&mut self, additional: usize) {
        self.buf.reserve(additional);
        self.sorted.reserve(additional);
    }

    /// Number of sprites currently in the buffer. / 缓冲区中当前的精灵数量。
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Push a sprite into the buffer. O(1) amortised. / 将精灵压入缓冲区。均摊 O(1)。
    pub fn push(&mut self, sprite: &Sprite2DObject<T, U>) {
        self.sorted.push(self.buf.len());
        self.buf.push(sprite.clone());
        self.buf_ver += 1;
    }

    /// Ensure the sorted index is up-to-date.
    /// 确保排序索引是最新的。
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

    /// Remove all sprites. O(n). / 移除所有精灵。O(n)。
    pub fn clear(&mut self) {
        self.buf.clear();
        self.sorted.clear();
        self.buf_ver = 0;
        self.sorted_ver = 0;
    }

    /// Iterate over items in sorted order.
    /// 按排序顺序迭代所有项。
    ///
    /// - `ex` — external context passed to both closures (e.g. a `SpriteBatch2D`) / 传递给两个 closure 的外部上下文
    /// - `on_pipeline_change` — called **before** the first item and whenever `pipeline.get_id()` changes / 首次及 pipeline 切换时调用
    /// - `on_item` — called for each sprite in sorted order / 按序每个精灵调用一次
    ///
    /// # Example / 示例
    ///
    /// ```ignore
    /// buf.for_each_sorted(
    ///     batch,
    ///     |batch, pipeline| {
    ///         batch.submit_and_draw(gfx).ok();
    ///         batch.clear_batch();
    ///         batch.set_texture(pipeline.0.srv.clone(), ...);
    ///     },
    ///     |batch, obj| {
    ///         batch.add(obj.transform.pos, ...);
    ///     },
    /// );
    /// ```
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