use glam::Vec2;
use krjw_engine::{AtlasText, Collider, ShapeBatch2D, Sprite2DBuffer, TextureInfoArced, Transform2D, atlas_text::TextLayout, cosmic_text::{Attrs, Metrics}, graphic};

/// 鱼朝向
#[derive(Clone, Copy, PartialEq)]
pub enum FishFacing {
    Right,
    Left
}

/// 🐟 鱼种类
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FishSpecies {
    Normal,     // 🐟 普通鱼
    Tropical,   // 🐠 热带鱼
    Puffer,     // 🐡 河豚
    Octopus,    // 🐙 章鱼
    Whale,      // 🐋 鲸鱼
    Shark,      // 🦈 鲨鱼
    Dolphin,    // 🐬 海豚
    Crab,       // 🦀 螃蟹
    Lobster,    // 🦞 龙虾
    Turtle,     // 🐢 海龟
    WaterHawk,  // 🦅 鹰
}

impl FishSpecies {
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Normal    => "🐟",
            Self::Tropical  => "🐠",
            Self::Puffer    => "🐡",
            Self::Octopus   => "🐙",
            Self::Whale     => "🐋",
            Self::Shark     => "🦈",
            Self::Dolphin   => "🐬",
            Self::Crab      => "🦀",
            Self::Lobster   => "🦞",
            Self::Turtle    => "🐢",
            Self::WaterHawk => "🦅",
        }
    }

    pub fn base_color(&self) -> [f32; 3] {
        match self {
            Self::Normal    => [0.6, 0.7, 0.9],
            Self::Tropical  => [1.0, 0.8, 0.7],
            Self::Puffer    => [0.6, 0.9, 0.6],
            Self::Octopus   => [0.9, 0.4, 0.7],
            Self::Whale     => [0.5, 0.6, 0.7],
            Self::Shark     => [0.5, 0.5, 0.6],
            Self::Dolphin   => [0.3, 0.6, 1.0],
            Self::Crab      => [1.0, 0.4, 0.3],
            Self::Lobster   => [0.8, 0.6, 0.6],
            Self::Turtle    => [0.5, 0.8, 0.5],
            Self::WaterHawk => [1.0, 1.0, 1.0],
        }
    }

    pub fn size_range(&self) -> (f32, f32) {
        match self {
            Self::Normal    => (6.0, 40.0),
            Self::Tropical  => (5.0, 32.0),
            Self::Puffer    => (24.0, 48.0),
            Self::Octopus   => (50.0, 72.0),
            Self::Whale     => (60.0, 120.0),
            Self::Shark     => (50.0, 100.0),
            Self::Dolphin   => (36.0, 72.0),
            Self::Crab      => (20.0, 36.0),
            Self::Lobster   => (30.0, 52.0),
            Self::Turtle    => (32.0, 64.0),
            Self::WaterHawk => (128.0, 256.0),
        }
    }

    pub fn origin_ratio(&self) -> Vec2 {
        match self {
            Self::Normal    => Vec2::new(0.625, 0.375),
            Self::Tropical  => Vec2::new(0.625, 0.370),
            Self::Puffer    => Vec2::new(0.625, 0.390),
            Self::Octopus   => Vec2::new(0.600, 0.350),
            Self::Whale     => Vec2::new(0.550, 0.375),
            Self::Shark     => Vec2::new(0.560, 0.375),
            Self::Dolphin   => Vec2::new(0.625, 0.370),
            Self::Crab      => Vec2::new(0.625, 0.380),
            Self::Lobster   => Vec2::new(0.600, 0.360),
            Self::Turtle    => Vec2::new(0.625, 0.370),
            Self::WaterHawk => Vec2::new(0.625, 0.320),
        }
    }

    /// 该种类解锁所需的玩家大小阈值
    pub fn unlock_size(&self) -> f32 {
        match self {
            Self::Normal    => 0.0,
            Self::Tropical  => 0.0,
            Self::Crab      => 42.0,
            Self::Puffer    => 42.0,
            Self::Dolphin   => 42.0,
            Self::Octopus   => 44.0,
            Self::Lobster   => 42.0,
            Self::Shark     => 42.0,
            Self::Turtle    => 44.0,
            Self::Whale     => 44.0,
            Self::WaterHawk => 50.0,
        }
    }

    /// 该种类最大生成速率（条/秒），达到 unlock_size 后逐渐达到此值
    pub fn max_spawn_rate(&self) -> f32 {
        match self {
            Self::Normal    => 0.3,
            Self::Tropical  => 0.3,
            Self::Puffer    => 0.3,
            Self::Octopus   => 0.15,
            Self::Whale     => 0.1,
            Self::Shark     => 0.1,
            Self::Dolphin   => 0.2,
            Self::Crab      => 0.2,
            Self::Lobster   => 0.1,
            Self::Turtle    => 0.15,
            Self::WaterHawk => 0.05,
        }
    }

    /// 被吃掉时的粒子颜色集合
    pub fn bitten_colors(&self) -> &[[f32; 3]] {
        match self {
            Self::Normal    => &[[0.6, 0.7, 0.9], [0.4, 0.5, 0.7], [0.8, 0.9, 1.0]],
            Self::Tropical  => &[[1.0, 0.7, 0.2], [1.0, 0.5, 0.0], [0.8, 0.6, 0.1]],
            Self::Puffer    => &[[0.6, 0.9, 0.3], [0.4, 0.7, 0.2], [0.8, 1.0, 0.5]],
            Self::Octopus   => &[[0.9, 0.4, 0.7], [0.7, 0.3, 0.5], [1.0, 0.5, 0.8]],
            Self::Whale     => &[[0.2, 0.3, 0.7], [0.1, 0.2, 0.5], [0.4, 0.5, 0.9]],
            Self::Shark     => &[[0.5, 0.5, 0.6], [0.4, 0.4, 0.5], [0.7, 0.7, 0.8]],
            Self::Dolphin   => &[[0.3, 0.6, 1.0], [0.2, 0.4, 0.8], [0.5, 0.8, 1.0]],
            Self::Crab      => &[[1.0, 0.4, 0.2], [0.8, 0.3, 0.1], [1.0, 0.6, 0.3]],
            Self::Lobster   => &[[0.8, 0.2, 0.2], [0.6, 0.1, 0.1], [1.0, 0.3, 0.3]],
            Self::Turtle    => &[[0.3, 0.6, 0.3], [0.2, 0.4, 0.2], [0.5, 0.8, 0.5]],
            Self::WaterHawk => &[[1.0, 1.0, 1.0], [0.9, 0.9, 0.9], [0.8, 0.8, 0.8]],
        }
    }
}

/// 🎭 行动模式
#[derive(Clone)]
pub enum MovementPattern {
    HorizontalEntry { from_left: bool, speed: f32 },
    VerticalEntry { from_top: bool, speed: f32 },
    Wave { speed: f32, amplitude: f32, frequency: f32, phase: f32, initial_x: f32, direction: f32 },
    Linear { velocity: Vec2 },
    Stationary,
}

/// 鱼消失淡出持续时间（秒）
const DISAPPEAR_DURATION: f32 = 1.0;

/// 🐟 鱼
pub struct Fish {
    pub species: FishSpecies,
    pub shape: String,
    pub shape_layout: TextLayout,
    pub size: f32,
    pub max_size: f32,
    pub facing: FishFacing,
    pub pos: glam::Vec2,
    pub color: [f32; 4],
    pub alpha: f32,
    pub movement: MovementPattern,
    pub origin: Vec2,
    pub eaten: bool,
    /// 入场淡入剩余时间（秒）
    pub spawn_fade: f32,
    /// 淡入总时长（秒）
    pub spawn_fade_duration: f32,
    pub rolling_speed: f32,
    pub rot: f32,
    /// 消失淡出剩余时间（秒），Some 表示正在淡出，None 表示正常状态
    pub disappear: Option<f32>,
    /// 存活年龄（秒），用于数量淘汰
    pub age: f32,
}

impl Fish {
    /// 创建玩家控制的鱼
    pub fn new(size: f32, max_size: f32, shape: &str, atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) -> Self {
        let max_size = max_size * 2.0;
        let shape_layout = atlas_text.layout_text(shape, Metrics::new(max_size, max_size), Attrs::new(), &gfx.device).unwrap();
        let shape = shape.to_string();
        Self {
            species: FishSpecies::Normal,
            origin: FishSpecies::Normal.origin_ratio() * max_size,
            shape,
            shape_layout,
            facing: FishFacing::Right,
            pos: Vec2::ZERO,
            size,
            max_size,
            color: [1.0, 1.0, 1.0, 1.0],
            alpha: 0.0,
            movement: MovementPattern::Stationary,
            eaten: false,
            spawn_fade: 1.0,
            spawn_fade_duration: 1.0,
            rolling_speed: if fastrand::f32() < 0.01 { 360.0_f32.to_radians() } else { 0.0 },
            rot: 0.0,
            disappear: None,
            age: 0.0,
        }
    }

    /// 创建一条自动鱼
    pub fn new_random_with_species(view_w: f32, view_h: f32, species: FishSpecies, atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) -> Self {
        use MovementPattern::*;
        let (min_s, max_s) = species.size_range();
        let size = if min_s >= max_s { min_s } else { min_s + fastrand::f32() * (max_s - min_s) };

        let base = species.base_color();
        let color_r = (base[0] + (fastrand::f32() - 0.5) * 0.3).clamp(0.0, 1.0);
        let color_g = (base[1] + (fastrand::f32() - 0.5) * 0.3).clamp(0.0, 1.0);
        let color_b = (base[2] + (fastrand::f32() - 0.5) * 0.3).clamp(0.0, 1.0);

        let pattern_roll = fastrand::f32();
        let half_w = view_w * 0.5;
        let half_h = view_h * 0.5;

        // 生成 movement、pos、facing 三元组
        let (pos, facing, movement) = if pattern_roll < 0.25 {
            let from_left = fastrand::f32() < 0.5;
            let y = (fastrand::f32() - 0.5) * view_h;
            let x = if from_left { -half_w - size } else { half_w + size };
            let facing = if from_left { FishFacing::Right } else { FishFacing::Left };
            (Vec2::new(x, y), facing, HorizontalEntry { from_left, speed: 50.0 + fastrand::f32() * 150.0 })
        } else if pattern_roll < 0.45 {
            let from_top = fastrand::f32() < 0.5;
            let x = (fastrand::f32() - 0.5) * view_w;
            let y = if from_top { -half_h - size } else { half_h + size };
            let facing = if fastrand::f32() < 0.5 { FishFacing::Left } else { FishFacing::Right };
            (Vec2::new(x, y), facing, VerticalEntry { from_top, speed: 40.0 + fastrand::f32() * 120.0 })
        } else if pattern_roll < 0.80 {
            let x = (fastrand::f32() - 0.5) * view_w;
            let y = (fastrand::f32() - 0.5) * view_h;
            let dir = if fastrand::f32() < 0.5 { 1.0 } else { -1.0 };
            let facing = if dir > 0.0 { FishFacing::Right } else { FishFacing::Left };
            (Vec2::new(x, y), facing, Wave { speed: 30.0 + fastrand::f32() * 100.0, amplitude: 20.0 + fastrand::f32() * 60.0, frequency: 1.0 + fastrand::f32() * 3.0, phase: fastrand::f32() * 6.28, initial_x: x, direction: dir })
        } else {
            let angle = fastrand::f32() * 6.28;
            let speed = 40.0 + fastrand::f32() * 120.0;
            let vel = Vec2::new(angle.cos() * speed, angle.sin() * speed);
            let facing = if vel.x > 0.0 { FishFacing::Right } else { FishFacing::Left };
            let x = (fastrand::f32() - 0.5) * view_w;
            let y = (fastrand::f32() - 0.5) * view_h;
            (Vec2::new(x, y), facing, Linear { velocity: vel })
        };

        let shape_str = species.emoji();
        let max_size = max_s * 2.0;
        let shape_layout = atlas_text.layout_text(shape_str, Metrics::new(max_size, max_size), Attrs::new(), &gfx.device).unwrap();
        let fade_dur = 0.6 + fastrand::f32() * 0.8;

        Self {
            species,
            origin: species.origin_ratio() * max_size,
            shape: shape_str.to_string(),
            shape_layout,
            facing, pos, size, max_size,
            color: [color_r, color_g, color_b, 1.0],
            alpha: 0.0,
            movement,
            eaten: false,
            spawn_fade: fade_dur,
            spawn_fade_duration: fade_dur,
            rolling_speed: if fastrand::f32() < 0.01 { 360.0_f32.to_radians() } else { 0.0 },
            rot: 0.0,
            disappear: None,
            age: 0.0,
        }
    }

    /// 开始消失淡出
    pub fn start_disappear(&mut self) {
        if self.disappear.is_none() && !self.eaten {
            self.disappear = Some(DISAPPEAR_DURATION);
        }
    }

    /// 是否正在淡出
    pub fn is_disappearing(&self) -> bool {
        self.disappear.is_some()
    }

    pub fn update(&mut self, dt: f32, view_w: f32, view_h: f32) {
        // 累加年龄
        self.age += dt;

        // 更新消失计时器
        if let Some(d) = self.disappear {
            let new_d = d - dt;
            if new_d <= 0.0 {
                self.eaten = true;
                self.disappear = None;
            } else {
                self.disappear = Some(new_d);
            }
        }

        // 如果已标记为被吃或消失完成，不更新运动
        if self.eaten {
            return;
        }

        // 入场淡入
        if self.spawn_fade > 0.0 {
            self.spawn_fade = (self.spawn_fade - dt).max(0.0);
            self.alpha = 1.0 - (self.spawn_fade / self.spawn_fade_duration);
        }

        use MovementPattern::*;
        self.rot += self.rolling_speed * dt;
        let half_w = view_w * 0.5;
        let half_h = view_h * 0.5;

        match &mut self.movement {
            Stationary => {}
            HorizontalEntry { from_left, speed } => {
                let dir = if *from_left { 1.0 } else { -1.0 };
                self.pos.x += dir * *speed * dt;
                self.facing = if *from_left { FishFacing::Right } else { FishFacing::Left };
                // 超出边界触发淡出
                if self.pos.x > half_w + self.size * 2.0 || self.pos.x < -half_w - self.size * 2.0 {
                    self.start_disappear();
                }
            }
            VerticalEntry { from_top, speed } => {
                let dir = if *from_top { 1.0 } else { -1.0 };
                self.pos.y += dir * *speed * dt;
                if self.pos.y > half_h + self.size * 2.0 || self.pos.y < -half_h - self.size * 2.0 {
                    self.start_disappear();
                }
            }
            Wave { speed, amplitude, frequency, phase, initial_x: _, direction } => {
                self.pos.x += *direction * *speed * dt;
                self.facing = if *direction > 0.0 { FishFacing::Right } else { FishFacing::Left };
                let wave_offset = (*amplitude) * (self.pos.x * *frequency * 0.01 + *phase).sin();
                self.pos.y += wave_offset * dt * 2.0;
                if self.pos.x > half_w + self.size * 2.0 || self.pos.x < -half_w - self.size * 2.0 {
                    self.start_disappear();
                }
            }
            Linear { velocity } => {
                self.pos += *velocity * dt;
                self.facing = if velocity.x > 0.0 { FishFacing::Right } else { FishFacing::Left };
                if self.pos.x < -half_w - self.size * 2.0 || self.pos.x > half_w + self.size * 2.0 ||
                   self.pos.y < -half_h - self.size * 2.0 || self.pos.y > half_h + self.size * 2.0 {
                    self.start_disappear();
                }
            }
        }
    }

    pub fn apply_hurt_flash(&mut self) { self.color = [1.0, 0.3, 0.3, 1.0]; }
    pub fn reset_color(&mut self) { self.color = [1.0, 1.0, 1.0, 1.0]; }

    pub fn set_invincible_flash(&mut self, invincible: f32) {
        if invincible > 0.0 {
            let blink = (invincible * 10.0) as i32 % 2 == 0;
            self.alpha = if blink { 1.0 } else { 0.2 };
        }
    }

    pub fn get_collider(&self) -> Collider {
        Collider::Circle { radius: self.size * 0.6 }
    }

    pub fn get_transform(&self) -> Transform2D {
        Transform2D {
            pos: self.pos,
            scale: match self.facing {
                FishFacing::Left => Vec2::ONE,
                FishFacing::Right => Vec2::new(-1.0, 1.0),
            } * self.size / self.max_size * 2.0,
            rot: self.rot,
        }
    }

    /// 获取消失因子（0~1），用于淡出透明度
    pub fn disappear_factor(&self) -> f32 {
        if let Some(d) = self.disappear {
            (d / DISAPPEAR_DURATION).max(0.0).min(1.0)
        } else {
            1.0
        }
    }

    pub fn add_to_buffer(&self, _gfx: &graphic::d3d11::D3D11, atlas_text: &mut AtlasText, sprite_buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>) {
        if self.eaten { return; }
        let disappear_factor = self.disappear_factor();
        let final_alpha = self.alpha * disappear_factor;
        let final_color = [self.color[0], self.color[1], self.color[2], self.color[3] * final_alpha];
        // 黑色阴影
        atlas_text.render_layout(&self.shape_layout, Vec2::ZERO, self.origin, self.get_transform().move_by(Vec2::new(5.0, 5.0)), [0.0, 0.0, 0.0, 0.3 * final_alpha], 0.0, sprite_buffer);
        // 本体
        atlas_text.render_layout(&self.shape_layout, Vec2::ZERO, self.origin, self.get_transform(), final_color, 0.0, sprite_buffer);
    }

    pub fn dbg_add_shape(&self, shape_batch_2d: &mut ShapeBatch2D) {
        if self.eaten { return; }
        let disappear_factor = self.disappear_factor();
        let final_alpha = self.alpha * disappear_factor;
        shape_batch_2d.add_circle_no_uv(self.pos, self.size * 0.6, [self.color[0], self.color[1], self.color[2], 0.5 * final_alpha], 24);
    }
}