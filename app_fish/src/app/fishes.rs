use super::fish::{Fish, FishSpecies};
use glam::Vec2;
use krjw_engine::{AtlasText, ShapeBatch2D, Sprite2DBuffer, TextureInfoArced, Transform2D, graphic};
use std::collections::HashMap;

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
    pub fn new(view_w: f32, view_h: f32) -> Self {
        Self {
            fish_list: Vec::new(),
            view_w, view_h,
            progress_size: 0.0,
            spawn_acc: HashMap::new(),
        }
    }

    pub fn set_view_size(&mut self, w: f32, h: f32) {
        self.view_w = w;
        self.view_h = h;
    }

    pub fn spawn_one_of_species(&mut self, species: FishSpecies, atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) {
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

            // 从 unlock 到 unlock+50 线性提升到 max_spawn_rate
            let progress = ((self.progress_size - unlock) / 50.0).min(1.0);
            let rate = progress * species.max_spawn_rate();

            if rate <= 0.0 { continue; }

            let acc = self.spawn_acc.entry(species).or_insert(0.0);
            *acc += rate * dt;

            let mut spawn_count = 0u32;
            while *acc >= 1.0 && self.fish_list.len() + needs_spawn.len() < 35 {
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