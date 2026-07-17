use glam::Vec2;
use krjw_engine::{
    atlas_text::TextLayout,
    cosmic_text::{Attrs, Metrics},
    graphic, AtlasText, Sprite2DBuffer, TextureInfoArced, Transform2D,
};
use std::collections::HashMap;

/// 物品最大数量上限
const MAX_ITEMS_COUNT: usize = 30;
/// 物品消失淡出持续时间（秒）
const DISAPPEAR_DURATION: f32 = 0.6;

// ============================================================
// 移动模式（与鱼群共用）
// ============================================================
#[derive(Clone, Debug)]
pub enum MovementPattern {
    HorizontalEntry { from_left: bool, speed: f32 },
    VerticalEntry { from_top: bool, speed: f32 },
    Wave {
        speed: f32,
        amplitude: f32,
        frequency: f32,
        phase: f32,
        direction: f32,
    },
    Linear { velocity: Vec2 },
    Stationary,
}

impl MovementPattern {
    pub fn random_new() -> Self {
        let choice = fastrand::u32(0..5);
        match choice {
            0 => {
                let from_left = fastrand::bool();
                let speed = 50.0 + fastrand::f32() * 150.0;
                Self::HorizontalEntry { from_left, speed }
            }
            1 => {
                let from_top = fastrand::bool();
                let speed = 40.0 + fastrand::f32() * 120.0;
                Self::VerticalEntry { from_top, speed }
            }
            2 => {
                let speed = 30.0 + fastrand::f32() * 100.0;
                let amplitude = 20.0 + fastrand::f32() * 60.0;
                let frequency = 1.0 + fastrand::f32() * 3.0;
                let phase = fastrand::f32() * 6.28;
                let direction = if fastrand::bool() { 1.0 } else { -1.0 };
                Self::Wave {
                    speed,
                    amplitude,
                    frequency,
                    phase,
                    direction,
                }
            }
            3 => {
                let angle = fastrand::f32() * 6.28;
                let spd = 40.0 + fastrand::f32() * 120.0;
                let velocity = Vec2::new(angle.cos() * spd, angle.sin() * spd);
                Self::Linear { velocity }
            }
            _ => Self::Stationary,
        }
    }

    pub fn random_new_pos(&self, view_w: f32, view_h: f32, size: f32) -> Vec2 {
        let half_w = view_w * 0.5;
        let half_h = view_h * 0.5;

        match self {
            Self::HorizontalEntry { from_left, .. } => {
                let x = if *from_left { -half_w - size } else { half_w + size };
                let y = (fastrand::f32() - 0.5) * view_h;
                Vec2::new(x, y)
            }
            Self::VerticalEntry { from_top, .. } => {
                let y = if *from_top { -half_h - size } else { half_h + size };
                let x = (fastrand::f32() - 0.5) * view_w;
                Vec2::new(x, y)
            }
            Self::Wave { direction, .. } => {
                let x = if *direction > 0.0 { -half_w - size } else { half_w + size };
                let y = (fastrand::f32() - 0.5) * view_h;
                Vec2::new(x, y)
            }
            Self::Linear { .. } => {
                let edge = fastrand::u32(0..4);
                match edge {
                    0 => Vec2::new(-half_w - size, (fastrand::f32() - 0.5) * view_h),
                    1 => Vec2::new(half_w + size, (fastrand::f32() - 0.5) * view_h),
                    2 => Vec2::new((fastrand::f32() - 0.5) * view_w, -half_h - size),
                    _ => Vec2::new((fastrand::f32() - 0.5) * view_w, half_h + size),
                }
            }
            Self::Stationary => {
                Vec2::new((fastrand::f32() - 0.5) * view_w, (fastrand::f32() - 0.5) * view_h)
            }
        }
    }
}

// ============================================================
// 物品类型定义 —— 所有配置均在此内部
// ============================================================
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    /// 减小碰到的玩家鱼的大小，但是增加该玩家鱼的生命值
    SizeToLife,
    /// 交换两个玩家的大小
    SizeSwap,
    /// 旋转无敌：触碰后玩家鱼旋转且无敌，可吃2倍大鱼
    InvincibleRotate,
}

impl Item {
    /// 所有物品类型的列表（用于遍历）
    pub const ALL: &'static [Item] = &[
        Item::SizeToLife,
        Item::SizeSwap,
        Item::InvincibleRotate,
    ];

    // ─── 基础属性 ──────────────────────────────────────────────

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::SizeToLife => "🩸",
            Self::SizeSwap => "🧾",
            Self::InvincibleRotate => "🌀",
        }
    }

    pub fn max_render_size(&self) -> f32 {
        match self {
            Self::SizeToLife => 60.0,
            Self::SizeSwap => 60.0,
            Self::InvincibleRotate => 60.0,
        }
    }

    pub fn random_size(&self) -> f32 {
        match self {
            Self::SizeToLife => 35.0 + fastrand::f32() * 25.0,
            Self::SizeSwap => 38.0 + fastrand::f32() * 22.0,
            Self::InvincibleRotate => 34.0 + fastrand::f32() * 20.0,
        }
    }

    pub fn color(&self) -> [f32; 4] {
        match self {
            Self::SizeToLife => [1.0, 0.2, 0.2, 1.0],
            Self::SizeSwap => [0.8, 0.8, 1.0, 1.0],
            Self::InvincibleRotate => [0.8, 0.4, 1.0, 1.0], // 紫色
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::SizeToLife => "Size→Life",
            Self::SizeSwap => "Size Swap",
            Self::InvincibleRotate => "Rotate",
        }
    }

    // ─── 生成配置（权重、解锁条件、生成速率） ────────────────

    pub fn weight(&self) -> f32 {
        match self {
            Self::SizeToLife => 1.0,
            Self::SizeSwap => 0.8,
            Self::InvincibleRotate => 0.6,
        }
    }

    pub fn unlock_threshold(&self) -> f32 {
        match self {
            Self::SizeToLife => 42.0,
            Self::SizeSwap => 42.0,
            Self::InvincibleRotate => 40.0,
        }
    }

    pub fn base_spawn_rate(&self) -> f32 {
        match self {
            Self::SizeToLife => 0.05,
            Self::SizeSwap => 0.05,
            Self::InvincibleRotate => 0.05,
        }
    }

    // ─── 辅助方法 ──────────────────────────────────────────────

    pub fn all_weights() -> Vec<f32> {
        Self::ALL.iter().map(|item| item.weight()).collect()
    }

    pub fn random_weighted() -> Self {
        let weights = Self::all_weights();
        let total: f32 = weights.iter().sum();
        let mut r = fastrand::f32() * total;
        for (idx, &w) in weights.iter().enumerate() {
            if r < w {
                return Self::ALL[idx].clone();
            }
            r -= w;
        }
        Self::ALL.last().unwrap().clone()
    }

    #[allow(unused)]
    pub fn random() -> Self {
        let idx = fastrand::usize(0..Self::ALL.len());
        Self::ALL[idx].clone()
    }
}

// ============================================================
// 物品容器 —— 自动管理生成和更新
// ============================================================
#[derive(Default, Debug)]
pub struct Items {
    // ---- 数据 ----
    len: usize,
    i_pos: Vec<Vec2>,
    i_alpha: Vec<f32>,
    i_move: Vec<MovementPattern>,
    i_size: Vec<f32>,
    i_type: Vec<Item>,
    i_pltouched: Vec<bool>,
    i_hint: Vec<bool>,
    i_fade_elapsed: Vec<f32>,
    i_age: Vec<f32>,
    i_disappear: Vec<Option<f32>>,
    i_pending_remove: Vec<bool>,

    // ---- 布局缓存 ----
    layouts: HashMap<Item, TextLayout>,
    layout_render_size: f32,

    // ---- 自动生成控制 ----
    pub auto_spawn_enabled: bool,
    spawn_timer: f32,
    spawn_interval: f32,
    pub max_spawn_rate: f32,
    pub progress_size: f32,
    pub hinted: bool,
}

impl Items {
    pub fn dbg_info(&self) -> String {
        let disappearing = self.i_disappear.iter().filter(|d| d.is_some()).count();
        let fading_in = self.i_alpha.iter().filter(|&&a| a < 1.0 && a > 0.0).count();
        let pending_remove = self.i_pending_remove.iter().filter(|&&p| p).count();
        let touched = self.i_pltouched.iter().filter(|&&t| t).count();

        let mut type_count = std::collections::BTreeMap::new();
        for i in 0..self.len {
            if !self.i_pltouched[i] && !self.i_pending_remove[i] {
                *type_count.entry(self.i_type[i].name()).or_insert(0) += 1;
            }
        }
        let type_dist: Vec<String> = type_count
            .iter()
            .map(|(name, count)| format!("{}:{}", name, count))
            .collect();

        let mut total_rate = 0.0;
        for item in Item::ALL {
            if self.progress_size >= item.unlock_threshold() {
                total_rate += item.base_spawn_rate();
            }
        }
        let progress_factor = (self.progress_size / 100.0).min(1.0);
        let effective_rate = total_rate * (0.2 + 0.8 * progress_factor);

        format!(
            "📦 物品状态:\n\
        ├─ 总数: {} (淡入中: {}, 淡出中: {}, 待移除: {}, 已触碰: {})\n\
        ├─ 进度尺寸: {:.1}\n\
        ├─ 生成计时器: {:.2}/{:.2} (有效速率: {:.3}/s)\n\
        ├─ 各类型分布: {}\n\
        └─ 自动生成: {}",
            self.len,
            fading_in,
            disappearing,
            pending_remove,
            touched,
            self.progress_size,
            self.spawn_timer,
            self.spawn_interval,
            effective_rate,
            if type_dist.is_empty() {
                "无".to_string()
            } else {
                type_dist.join(", ")
            },
            if self.auto_spawn_enabled {
                "✅ 已启用"
            } else {
                "❌ 已禁用"
            }
        )
    }

    // ─── 初始化 ──────────────────────────────────────────────────

    pub fn init_layouts(&mut self, atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) {
        for item in Item::ALL {
            let render_size = item.max_render_size() * 2.0;
            let layout = atlas_text
                .layout_text(
                    item.emoji(),
                    Metrics::new(render_size, render_size),
                    Attrs::new(),
                    &gfx.device,
                )
                .unwrap_or_else(|_| panic!("Failed to layout emoji for {:?}", item));
            self.layouts.insert(item.clone(), layout);
            self.layout_render_size = render_size;
        }
        self.auto_spawn_enabled = true;
        self.spawn_interval = 5.0;
        self.max_spawn_rate = 0.5;
        self.progress_size = 0.0;
    }

    // ─── 添加/移除 ──────────────────────────────────────────────

    fn start_disappear(&mut self, index: usize) {
        if self.i_disappear[index].is_none() && !self.i_pltouched[index] {
            self.i_disappear[index] = Some(DISAPPEAR_DURATION);
        }
    }

    fn is_disappearing(&self, index: usize) -> bool {
        self.i_disappear[index].is_some()
    }

    fn remove_oldest(&mut self) {
        if self.len == 0 {
            return;
        }
        let mut oldest_idx = 0;
        let mut max_age = 0.0;
        for i in 0..self.len {
            if self.i_age[i] > max_age {
                max_age = self.i_age[i];
                oldest_idx = i;
            }
        }
        if self.is_disappearing(oldest_idx) {
            self.i_pending_remove[oldest_idx] = true;
        } else {
            self.start_disappear(oldest_idx);
        }
    }

    pub fn new_item(&mut self, item: Item, view_w: f32, view_h: f32) {
        if self.len >= MAX_ITEMS_COUNT {
            self.remove_oldest();
        }

        let size = item.random_size();
        let move_pattern = MovementPattern::random_new();
        let pos = move_pattern.random_new_pos(view_w, view_h, size);

        self.i_pos.push(pos);
        self.i_alpha.push(0.0);
        self.i_move.push(move_pattern);
        self.i_size.push(size);
        self.i_type.push(item);
        self.i_pltouched.push(false);
        self.i_fade_elapsed.push(0.0);
        self.i_hint.push(self.hinted);
        self.i_age.push(0.0);
        self.i_disappear.push(None);
        self.i_pending_remove.push(false);
        self.len += 1;
        self.hinted = true;
    }

    pub fn swap_remove_item(&mut self, index: usize) {
        assert!(index < self.len);
        self.i_pos.swap_remove(index);
        self.i_alpha.swap_remove(index);
        self.i_move.swap_remove(index);
        self.i_size.swap_remove(index);
        self.i_type.swap_remove(index);
        self.i_pltouched.swap_remove(index);
        self.i_fade_elapsed.swap_remove(index);
        self.i_hint.swap_remove(index);
        self.i_age.swap_remove(index);
        self.i_disappear.swap_remove(index);
        self.i_pending_remove.swap_remove(index);
        self.len -= 1;
    }

    #[allow(unused)]
    pub fn clear(&mut self) {
        self.i_pos.clear();
        self.i_alpha.clear();
        self.i_move.clear();
        self.i_size.clear();
        self.i_type.clear();
        self.i_pltouched.clear();
        self.i_fade_elapsed.clear();
        self.i_hint.clear();
        self.i_age.clear();
        self.i_disappear.clear();
        self.i_pending_remove.clear();
        self.len = 0;
    }

    // ─── 碰撞检测 ──────────────────────────────────────────────

    pub fn is_overlap(&self, index: usize, other_pos: Vec2, other_radius: f32) -> bool {
        let distance = self.i_pos[index].distance(other_pos);
        distance < self.i_size[index] + other_radius
    }

    pub fn foreach_overlap<T: FnMut(Vec2, f32, &MovementPattern, f32, &Item, bool) -> bool>(
        &mut self,
        other_pos: Vec2,
        other_radius: f32,
        mut on_touch: T,
    ) {
        for i in 0..self.len {
            if self.is_overlap(i, other_pos, other_radius) {
                let touched = on_touch(
                    self.i_pos[i],
                    self.i_alpha[i],
                    &self.i_move[i],
                    self.i_size[i],
                    &self.i_type[i],
                    self.i_pltouched[i],
                );
                self.i_pltouched[i] = touched;
            }
        }
    }

    // ─── 生命周期 ──────────────────────────────────────────────

    pub fn finished(&self, index: usize, _view_w: f32, _view_h: f32) -> bool {
        self.i_pltouched[index] || self.i_pending_remove[index]
    }

    pub fn clear_finished(&mut self, view_w: f32, view_h: f32) {
        let mut i = 0;
        while i < self.len {
            if self.finished(i, view_w, view_h) {
                self.swap_remove_item(i);
                continue;
            }
            i += 1;
        }
    }

    // ─── 运动更新 ──────────────────────────────────────────────

    pub fn process_motion_single(&mut self, index: usize, view_w: f32, view_h: f32, dt: f32) {
        if self.i_pending_remove[index] || self.i_pltouched[index] {
            return;
        }

        let pos = &mut self.i_pos[index];
        let mov = &mut self.i_move[index];
        let half_w = view_w * 0.5;
        let half_h = view_h * 0.5;
        let size = self.i_size[index];

        use MovementPattern::*;

        match mov {
            Stationary => {}
            HorizontalEntry { from_left, speed } => {
                let dir = if *from_left { 1.0 } else { -1.0 };
                pos.x += dir * *speed * dt;
                if pos.x > half_w + size * 2.0 || pos.x < -half_w - size * 2.0 {
                    self.start_disappear(index);
                }
            }
            VerticalEntry { from_top, speed } => {
                let dir = if *from_top { 1.0 } else { -1.0 };
                pos.y += dir * *speed * dt;
                if pos.y > half_h + size * 2.0 || pos.y < -half_h - size * 2.0 {
                    self.start_disappear(index);
                }
            }
            Wave {
                speed,
                amplitude,
                frequency,
                phase,
                direction,
            } => {
                pos.x += *direction * *speed * dt;
                let wave_offset = *amplitude * (pos.x * *frequency * 0.01 + *phase).sin();
                pos.y += wave_offset * dt * 2.0;
                if pos.x > half_w + size * 2.0 || pos.x < -half_w - size * 2.0 {
                    self.start_disappear(index);
                }
            }
            Linear { velocity } => {
                pos.x += velocity.x * dt;
                pos.y += velocity.y * dt;
                if pos.x < -half_w - size * 2.0
                    || pos.x > half_w + size * 2.0
                    || pos.y < -half_h - size * 2.0
                    || pos.y > half_h + size * 2.0
                {
                    self.start_disappear(index);
                }
            }
        }
    }

    pub fn process_motion(&mut self, view_w: f32, view_h: f32, dt: f32) {
        for i in 0..self.len {
            self.process_motion_single(i, view_w, view_h, dt);
        }
    }

    // ─── 自动生成逻辑 ──────────────────────────────────────────

    pub fn auto_spawn_step(&mut self, dt: f32, view_w: f32, view_h: f32) {
        if !self.auto_spawn_enabled || self.len >= MAX_ITEMS_COUNT {
            return;
        }

        let mut total_rate = 0.0;
        for item in Item::ALL {
            if self.progress_size >= item.unlock_threshold() {
                total_rate += item.base_spawn_rate();
            }
        }
        let progress_factor = (self.progress_size / 100.0).min(1.0);
        let effective_rate = total_rate * (0.2 + 0.8 * progress_factor);

        self.spawn_timer += dt * effective_rate;

        while self.spawn_timer >= 1.0 && self.len < MAX_ITEMS_COUNT {
            self.spawn_timer -= 1.0;
            let mut candidates = Vec::new();
            for item in Item::ALL {
                if self.progress_size >= item.unlock_threshold() {
                    candidates.push(item);
                }
            }
            if candidates.is_empty() {
                continue;
            }
            let total_weight: f32 = candidates.iter().map(|&item| item.weight()).sum();
            let mut r = fastrand::f32() * total_weight;
            let chosen = candidates
                .iter()
                .find(|&&item| {
                    let w = item.weight();
                    if r < w {
                        true
                    } else {
                        r -= w;
                        false
                    }
                })
                .unwrap_or(&candidates[0]);

            self.new_item((*chosen).clone(), view_w, view_h);
        }
    }

    // ─── 主更新 ──────────────────────────────────────────────────

    pub fn update_foreach(&mut self, dt: f64, view_w: f32, view_h: f32) {
        debug_assert_eq!(self.len, self.i_pos.len());
        debug_assert_eq!(self.len, self.i_alpha.len());
        debug_assert_eq!(self.len, self.i_move.len());
        debug_assert_eq!(self.len, self.i_type.len());
        debug_assert_eq!(self.len, self.i_size.len());
        debug_assert_eq!(self.len, self.i_pltouched.len());
        debug_assert_eq!(self.len, self.i_fade_elapsed.len());
        debug_assert_eq!(self.len, self.i_hint.len());
        debug_assert_eq!(self.len, self.i_age.len());
        debug_assert_eq!(self.len, self.i_disappear.len());
        debug_assert_eq!(self.len, self.i_pending_remove.len());

        let dt_f32 = dt as f32;
        const FADE_DURATION: f32 = 2.0;

        for i in 0..self.len {
            self.i_age[i] += dt_f32;

            if let Some(d) = self.i_disappear[i] {
                let new_d = d - dt_f32;
                if new_d <= 0.0 {
                    self.i_disappear[i] = None;
                    self.i_pending_remove[i] = true;
                } else {
                    self.i_disappear[i] = Some(new_d);
                }
            }

            if self.i_alpha[i] < 1.0 {
                self.i_fade_elapsed[i] += dt_f32;
                let progress = (self.i_fade_elapsed[i] / FADE_DURATION).min(1.0);
                self.i_alpha[i] = progress;
            }
        }

        self.process_motion(view_w, view_h, dt_f32);
        self.auto_spawn_step(dt_f32, view_w, view_h);
        self.clear_finished(view_w, view_h);
    }

    // ─── 绘制 ──────────────────────────────────────────────────

    fn get_layout(&self, item: &Item) -> &TextLayout {
        self.layouts
            .get(item)
            .unwrap_or_else(|| panic!("Layout for {:?} not initialized", item))
    }

    fn disappear_factor(&self, index: usize) -> f32 {
        if let Some(d) = self.i_disappear[index] {
            (d / DISAPPEAR_DURATION).max(0.0).min(1.0)
        } else {
            1.0
        }
    }

    pub fn draw_all(
        &self,
        atlas_text: &mut AtlasText,
        sprite_buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>,
    ) {
        for i in 0..self.len {
            if self.i_pltouched[i] || self.i_pending_remove[i] {
                continue;
            }
            let item_type = &self.i_type[i];
            let layout = self.get_layout(item_type);
            let pos = self.i_pos[i];
            let alpha = self.i_alpha[i];
            let disappear_factor = self.disappear_factor(i);
            let final_alpha = alpha * disappear_factor;
            let size = self.i_size[i];

            let layout_render_size = item_type.max_render_size() * 2.0;
            let scale = size / layout_render_size;
            let transform = Transform2D {
                pos,
                scale: Vec2::new(scale, scale),
                rot: 0.0,
            };

            let origin = Vec2::new(layout_render_size * 0.5, layout_render_size * 0.5);
            let base_color = item_type.color();
            let color = [base_color[0], base_color[1], base_color[2], base_color[3] * final_alpha];

            atlas_text.render_layout(
                layout,
                Vec2::ZERO,
                origin,
                transform. with_move_by(Vec2::new(5.0, 5.0)),
                [0.0, 0.0, 0.0, 0.5 * final_alpha],
                0.0,
                sprite_buffer,
            );
            atlas_text.render_layout(
                layout,
                Vec2::ZERO,
                origin,
                transform,
                color,
                0.0,
                sprite_buffer,
            );
        }
    }

    // ─── 辅助 ──────────────────────────────────────────────────

    #[allow(unused)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn set_progress(&mut self, total_size: f32) {
        self.progress_size = total_size;
    }
}