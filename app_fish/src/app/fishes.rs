use super::fish::{Fish, FishSpecies};
use glam::Vec2;
use krjw_engine::{AtlasText, ShapeBatch2D, Sprite2DBuffer, TextureInfoArced, Transform2D, graphic};
use std::collections::HashMap;

/// 自动鱼最大数量上限
const MAX_FISH_COUNT: usize = 200;

pub struct Fishes {
    pub fish_list: Vec<Fish>,
    view_w: f32,
    view_h: f32,
    /// 游戏进度 = 两个玩家 size 的最大值
    pub progress_size: f32,
    spawn_acc: HashMap<FishSpecies, f32>,
}

pub struct EatResult {
    pub eaten_count: usize,
    pub hit_by_big: bool,
    pub last_eaten_species: Option<FishSpecies>,
    /// 最后被吃掉的鱼的大小（用于粒子缩放）
    pub last_eaten_size: f32,
}

impl Fishes {
    pub fn new() -> Self {
        Self {
            fish_list: Vec::new(),
            view_w: 0.0, view_h: 0.0,
            progress_size: 0.0,
            spawn_acc: HashMap::new(),
        }
    }

    pub fn dbg_info(&self) -> String {
        let total = self.fish_list.len();
        let disappearing = self.fish_list.iter().filter(|f| f.is_disappearing()).count();
        let fading_in = self.fish_list.iter().filter(|f| f.spawn_fade > 0.0).count();
        
        let species_count: Vec<String> = self.spawn_acc.iter()
            .map(|(species, acc)| format!("{:?}:{:.2}", species, acc))
            .collect();
        
        format!(
            "鱼群状态:\n\
            　总数: {} (淡入中: {}, 淡出中: {})\n\
            　进度尺寸: {:.1}\n\
            　生成累计: {}\n\
            　各物种累计: {}",
            total,
            fading_in,
            disappearing,
            self.progress_size,
            self.spawn_acc.values().sum::<f32>(),
            species_count.join(", ")
        )
    }

    pub fn set_view_size(&mut self, w: f32, h: f32) {
        self.view_w = w;
        self.view_h = h;
    }

    /// 移除最旧的鱼（年龄最大），若已在淡出则直接标记移除，否则触发淡出
    fn remove_oldest(&mut self) {
        if self.fish_list.is_empty() { return; }
        let mut oldest_idx = 0;
        let mut max_age = 0.0;
        for (i, fish) in self.fish_list.iter().enumerate() {
            if fish.age > max_age {
                max_age = fish.age;
                oldest_idx = i;
            }
        }
        let fish = &mut self.fish_list[oldest_idx];
        if fish.is_disappearing() {
            // 已在淡出，直接标记移除
            fish.eaten = true;
        } else {
            fish.start_disappear();
        }
    }

    pub fn spawn_one_of_species(&mut self, species: FishSpecies, atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) {
        // 确保不超出数量上限
        if self.fish_list.len() >= MAX_FISH_COUNT {
            self.remove_oldest();
        }
        let fish = Fish::new_random_with_species(self.view_w, self.view_h, species, atlas_text, gfx);
        self.fish_list.push(fish);
    }

    fn try_spawn(&mut self, dt: f32, atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) {
        use FishSpecies::*;
        const ALL_SPECIES: &[FishSpecies] = &[
            Normal, Tropical, Puffer, Octopus, Whale, Shark, Dolphin, Crab, Lobster, Turtle, WaterHawk,
        ];

        let mut needs_spawn: Vec<(FishSpecies, u32)> = Vec::new();

        for &species in ALL_SPECIES {
            let unlock = species.unlock_size();
            // 如果进度未达到解锁尺寸，跳过
            if self.progress_size < unlock {
                continue;
            }

            // 从 unlock 到 unlock+25 线性提升到 max_spawn_rate
            let progress = ((self.progress_size - unlock) / 25.0).min(1.0);
            let rate = progress * species.max_spawn_rate();

            if rate <= 0.0 { continue; }

            let acc = self.spawn_acc.entry(species).or_insert(0.0);
            *acc += rate * dt;

            let mut spawn_count = 0u32;
            while *acc >= 1.0 && self.fish_list.len() + needs_spawn.len() < MAX_FISH_COUNT {
                *acc -= 1.0;
                spawn_count += 1;
            }
            if spawn_count > 0 {
                needs_spawn.push((species, spawn_count));
            }
        }

        for (species, count) in needs_spawn {
            for _ in 0..count {
                self.spawn_one_of_species(species, atlas_text, gfx);
            }
        }
    }

    pub fn check_interact(&mut self, player_pos: Vec2, player_size: f32) -> EatResult {
        let player_radius = player_size * 0.6;
        let mut eaten_count = 0;
        let mut hit_by_big = false;
        let mut to_remove = Vec::new();
        let mut last_species = None;
        let mut last_size = 0.0;

        for (i, fish) in self.fish_list.iter().enumerate() {
            if fish.eaten { continue; }
            // 淡入中的鱼无法交互（既不能被吃也不能伤害玩家）
            if fish.spawn_fade > 0.0 { continue; }
            // 正在淡出的鱼仍然可以交互（玩家可以吃掉它）
            let d = fish.pos - player_pos;
            let r_sum = fish.size * 0.6 + player_radius;
            if d.length_squared() > r_sum * r_sum { continue; }

            if fish.size < player_size {
                last_species = Some(fish.species);
                last_size = fish.size;
                eaten_count += 1;
                to_remove.push(i);
            } else {
                hit_by_big = true;
            }
        }

        for i in to_remove.into_iter().rev() {
            self.fish_list.swap_remove(i);
        }

        EatResult { eaten_count, hit_by_big, last_eaten_species: last_species, last_eaten_size: last_size }
    }

    pub fn update(&mut self, dt: f32, atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) {
        self.try_spawn(dt, atlas_text, gfx);
        for fish in &mut self.fish_list {
            fish.update(dt, self.view_w, self.view_h);
        }
        // 移除被吃或消失完成的鱼
        self.fish_list.retain(|fish| !fish.eaten);
    }

    pub fn add_to_buffer(&self, gfx: &graphic::d3d11::D3D11, atlas_text: &mut AtlasText, sprite_buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>) {
        for fish in &self.fish_list {
            fish.add_to_buffer(gfx, atlas_text, sprite_buffer);
        }
    }

    pub fn dbg_add_shapes(&self, shape_batch: &mut ShapeBatch2D) {
        for fish in &self.fish_list {
            fish.dbg_add_shape(shape_batch);
        }
    }
}