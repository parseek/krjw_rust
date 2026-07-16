use glam::Vec2;
use krjw_engine::{
    atlas_text::TextLayout,
    cosmic_text::{Attrs, Metrics},
    graphic, AtlasText, Sprite2DBuffer, TextureInfoArced, Transform2D,
};
use std::collections::HashMap;

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
    pub fn random_new(view_w: f32, view_h: f32) -> Self {
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
    // 未来可添加：
    // SpeedBoost,
    // Invincibility,
    // Magnet,
    // etc.
}

impl Item {
    /// 所有物品类型的列表（用于遍历）
    pub const ALL: &'static [Item] = &[
        Item::SizeToLife,
        Item::SizeSwap,
    ];

    // ─── 基础属性 ──────────────────────────────────────────────

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::SizeToLife => "🩸",
            Self::SizeSwap => "🧾",
        }
    }

    pub fn max_render_size(&self) -> f32 {
        match self {
            Self::SizeToLife => 40.0,
            Self::SizeSwap => 40.0,
        }
    }

    pub fn random_size(&self) -> f32 {
        match self {
            Self::SizeToLife => 15.0 + fastrand::f32() * 25.0,
            Self::SizeSwap => 18.0 + fastrand::f32() * 22.0,
        }
    }

    pub fn color(&self) -> [f32; 4] {
        match self {
            Self::SizeToLife => [1.0, 0.2, 0.2, 1.0],
            Self::SizeSwap => [0.8, 0.8, 1.0, 1.0],
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::SizeToLife => "Size→Life",
            Self::SizeSwap => "Size Swap",
        }
    }

    // ─── 生成配置（权重、解锁条件、生成速率） ────────────────

    /// 权重（用于随机选择，值越大出现概率越高）
    pub fn weight(&self) -> f32 {
        match self {
            Self::SizeToLife => 1.0,
            Self::SizeSwap => 0.8,
        }
    }

    /// 解锁所需的最小玩家总大小（两个玩家之和）
    /// 如果总大小小于此值，该物品不会生成
    pub fn unlock_threshold(&self) -> f32 {
        match self {
            Self::SizeToLife => 70.0,   
            Self::SizeSwap => 70.0,
        }
    }

    /// 基础生成速率（每秒生成次数，在解锁后全速）
    /// 实际生成率会乘以一个进度因子（见 Items 中的计算）
    pub fn base_spawn_rate(&self) -> f32 {
        match self {
            Self::SizeToLife => 0.02,
            Self::SizeSwap => 0.02,
        }
    }

    // ─── 辅助方法 ──────────────────────────────────────────────

    /// 获取所有物品的权重列表（顺序与 ALL 一致）
    pub fn all_weights() -> Vec<f32> {
        Self::ALL.iter().map(|item| item.weight()).collect()
    }

    /// 带权重的随机选择（使用内部权重）
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

    /// 等概率随机（仅用于调试）
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
    i_fade_elapsed: Vec<f32>,

    // ---- 布局缓存 ----
    layouts: HashMap<Item, TextLayout>,
    layout_render_size: f32,

    // ---- 自动生成控制 ----
    pub auto_spawn_enabled: bool,
    spawn_timer: f32,
    spawn_interval: f32,       // 基础间隔，实际会动态调整
    pub max_spawn_rate: f32,    // 最大总生成速率（每秒），由外部进度调节
    pub progress_size: f32,     // 玩家总大小，用于解锁和速率调节
}

impl Items {
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
        // 默认启用自动生成
        self.auto_spawn_enabled = true;
        self.spawn_interval = 5.0; // 基础间隔，会根据进度调整
        self.max_spawn_rate = 0.5; // 每秒最多生成 0.5 个
        self.progress_size = 0.0;
    }

    // ─── 添加/移除 ──────────────────────────────────────────────

    pub fn new_item(&mut self, item: Item, view_w: f32, view_h: f32) {
        let size = item.random_size();
        let move_pattern = MovementPattern::random_new(view_w, view_h);
        let pos = move_pattern.random_new_pos(view_w, view_h, size);

        self.i_pos.push(pos);
        self.i_alpha.push(0.0);
        self.i_move.push(move_pattern);
        self.i_size.push(size);
        self.i_type.push(item);
        self.i_pltouched.push(false);
        self.i_fade_elapsed.push(0.0);
        self.len += 1;
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
        self.len -= 1;
    }

    pub fn clear(&mut self) {
        self.i_pos.clear();
        self.i_alpha.clear();
        self.i_move.clear();
        self.i_size.clear();
        self.i_type.clear();
        self.i_pltouched.clear();
        self.i_fade_elapsed.clear();
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
        self.i_pltouched[index]
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
                if pos.x > half_w + size || pos.x < -half_w - size {
                    *from_left = fastrand::bool();
                    pos.y = (fastrand::f32() - 0.5) * view_h;
                    pos.x = if *from_left { -half_w - size } else { half_w + size };
                    *speed = 50.0 + fastrand::f32() * 150.0;
                }
            }
            VerticalEntry { from_top, speed } => {
                let dir = if *from_top { 1.0 } else { -1.0 };
                pos.y += dir * *speed * dt;
                if pos.y > half_h + size || pos.y < -half_h - size {
                    *from_top = fastrand::bool();
                    pos.x = (fastrand::f32() - 0.5) * view_w;
                    pos.y = if *from_top { -half_h - size } else { half_h + size };
                    *speed = 40.0 + fastrand::f32() * 120.0;
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
                if pos.x > half_w + size || pos.x < -half_w - size {
                    *direction = if pos.x > 0.0 { -1.0 } else { 1.0 };
                    pos.y = (fastrand::f32() - 0.5) * view_h;
                    pos.x = if *direction > 0.0 { -half_w - size } else { half_w + size };
                    *speed = 30.0 + fastrand::f32() * 100.0;
                    *amplitude = 20.0 + fastrand::f32() * 60.0;
                    *frequency = 1.0 + fastrand::f32() * 3.0;
                    *phase = fastrand::f32() * 6.28;
                }
            }
            Linear { velocity } => {
                pos.x += velocity.x * dt;
                pos.y += velocity.y * dt;
                if pos.x < -half_w - size
                    || pos.x > half_w + size
                    || pos.y < -half_h - size
                    || pos.y > half_h + size
                {
                    let angle = fastrand::f32() * 6.28;
                    let spd = 40.0 + fastrand::f32() * 120.0;
                    *velocity = Vec2::new(angle.cos() * spd, angle.sin() * spd);
                    let edge = fastrand::u32(0..4);
                    match edge {
                        0 => *pos = Vec2::new(-half_w - size, (fastrand::f32() - 0.5) * view_h),
                        1 => *pos = Vec2::new(half_w + size, (fastrand::f32() - 0.5) * view_h),
                        2 => *pos = Vec2::new((fastrand::f32() - 0.5) * view_w, -half_h - size),
                        _ => *pos = Vec2::new((fastrand::f32() - 0.5) * view_w, half_h + size),
                    }
                }
            }
        }
    }

    pub fn process_motion(&mut self, view_w: f32, view_h: f32, dt: f32) {
        for i in 0..self.len {
            self.process_motion_single(i, view_w, view_h, dt);
        }
    }

    // ─── 自动生成逻辑（完全内置） ──────────────────────────────

    /// 尝试生成一个物品，基于当前进度和权重
    fn try_auto_spawn(&mut self, view_w: f32, view_h: f32) {
        if !self.auto_spawn_enabled || self.len >= 50 {
            return; // 上限防止过多
        }

        // 计算当前总生成速率（基于进度）
        let mut total_rate = 0.0;
        for item in Item::ALL {
            if self.progress_size >= item.unlock_threshold() {
                total_rate += item.base_spawn_rate();
            }
        }
        // 进度因子：0~1 之间，随着 progress_size 增加而增加
        let progress_factor = (self.progress_size / 100.0).min(1.0);
        let effective_rate = total_rate * (0.2 + 0.8 * progress_factor);
        let effective_interval = 1.0 / effective_rate.max(0.001);

        // 更新间隔（平滑变化）
        self.spawn_interval = self.spawn_interval * 0.9 + effective_interval * 0.1;

        // 计时器累加
        let dt = 1.0 / 60.0; // 假设每帧调用，实际 dt 由外部传入，但我们在 update 中处理
        // 但我们将在 update_foreach 中统一处理，这里只做决策
    }

    /// 外部调用，传入帧时间
    pub fn auto_spawn_step(&mut self, dt: f32, view_w: f32, view_h: f32) {
        if !self.auto_spawn_enabled || self.len >= 50 {
            return;
        }

        // 计算当前总生成速率
        let mut total_rate = 0.0;
        for item in Item::ALL {
            if self.progress_size >= item.unlock_threshold() {
                total_rate += item.base_spawn_rate();
            }
        }
        let progress_factor = (self.progress_size / 100.0).min(1.0);
        let effective_rate = total_rate * (0.2 + 0.8 * progress_factor);

        // 累加计时器
        self.spawn_timer += dt * effective_rate;

        // 当累积到 1 时生成
        while self.spawn_timer >= 1.0 {
            self.spawn_timer -= 1.0;
            // 选择物品类型（基于权重，且只考虑已解锁的）
            let mut candidates = Vec::new();
            for item in Item::ALL {
                if self.progress_size >= item.unlock_threshold() {
                    candidates.push(item);
                }
            }
            if candidates.is_empty() {
                continue;
            }
            // 根据权重随机选择
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

    // ─── 主更新（外部每帧调用） ────────────────────────────────

    pub fn update_foreach(&mut self, dt: f64, view_w: f32, view_h: f32) {
        debug_assert_eq!(self.len, self.i_pos.len());
        debug_assert_eq!(self.len, self.i_alpha.len());
        debug_assert_eq!(self.len, self.i_move.len());
        debug_assert_eq!(self.len, self.i_type.len());
        debug_assert_eq!(self.len, self.i_size.len());
        debug_assert_eq!(self.len, self.i_pltouched.len());
        debug_assert_eq!(self.len, self.i_fade_elapsed.len());

        let dt_f32 = dt as f32;
        const FADE_DURATION: f32 = 2.0;

        // 1. 淡入
        for i in 0..self.len {
            if self.i_alpha[i] < 1.0 {
                self.i_fade_elapsed[i] += dt_f32;
                let progress = (self.i_fade_elapsed[i] / FADE_DURATION).min(1.0);
                self.i_alpha[i] = progress;
            }
        }

        // 2. 运动更新
        self.process_motion(view_w, view_h, dt_f32);

        // 3. 自动生成（内部使用 progress_size）
        self.auto_spawn_step(dt_f32, view_w, view_h);

        // 4. 清理被触碰的物品
        self.clear_finished(view_w, view_h);
    }

    // ─── 绘制 ──────────────────────────────────────────────────

    fn get_layout(&self, item: &Item) -> &TextLayout {
        self.layouts
            .get(item)
            .unwrap_or_else(|| panic!("Layout for {:?} not initialized", item))
    }

    pub fn draw_all(
        &self,
        atlas_text: &mut AtlasText,
        sprite_buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>,
    ) {
        for i in 0..self.len {
            if self.i_pltouched[i] {
                continue;
            }
            let item_type = &self.i_type[i];
            let layout = self.get_layout(item_type);
            let pos = self.i_pos[i];
            let alpha = self.i_alpha[i];
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
            let color = [base_color[0], base_color[1], base_color[2], base_color[3] * alpha];

            // 阴影
            atlas_text.render_layout(
                layout,
                Vec2::ZERO,
                origin,
                transform.move_by(Vec2::new(5.0, 5.0)),
                [0.0, 0.0, 0.0, 0.5 * alpha],
                0.0,
                sprite_buffer,
            );
            // 本体
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

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 更新进度（外部在游戏循环中调用，通常由玩家总大小决定）
    pub fn set_progress(&mut self, total_size: f32) {
        self.progress_size = total_size;
    }
}