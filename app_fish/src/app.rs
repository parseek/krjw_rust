use anyhow::{Context, Result};
use glam::Vec2;
use kira::sound::static_sound::StaticSoundData;
use krjw_engine::{cosmic_text::{Attrs, Metrics}, winit::keyboard::KeyCode, *};
use krjw_engine::macros::*;
use std::{collections::HashMap, io::Cursor, sync::mpsc::Receiver};

mod grid_render;
mod fish;
mod fishes;
mod items;

mod helper_window;

use fish::ALL_FISH_SPECIES;

/// 🎉 粒子 —— 吃鱼时的视觉效果
pub struct Particle {
    pos: Vec2,
    vel: Vec2,
    lifetime: f32,
    max_lifetime: f32,
    size: f32,
    start_size: f32,
    color: [f32; 4],
}

impl Particle {
    fn update(&mut self, dt: f32) {
        self.pos += self.vel * dt;
        self.vel *= 0.95; // 摩擦力衰减
        self.lifetime -= dt;
    }

    fn alive(&self) -> bool { self.lifetime > 0.0 }

    fn alpha(&self) -> f32 {
        (self.lifetime / self.max_lifetime).max(0.0)
    }
}

/// ✨ 浮动得分动画
pub struct ScorePopup {
    pos: Vec2,
    lifetime: f32,
}

pub struct App {
    pub ctx: Option<AppContext>,
}

pub struct AppContext {
    pub window: winit::window::Window,
    pub driver: EventDriver,
    pub gfx: D3D11,
    pub sprite_batch: SpriteBatch2D,
    pub shape_batch: ShapeBatch2D,
    pub camera: Camera2D,
    pub timer: Timer,
    pub sprite_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    pub atlas_text: AtlasText,
    pub time_elapsed: f64,
    pub audio: kira::AudioManager,
    pub sounds: HashMap<String, StaticSoundData>,

    pub player1_fish: fish::Fish,
    pub player2_fish: fish::Fish,
    pub fishes: fishes::Fishes,

    // 生命值
    pub p1_lives: i32,
    pub p2_lives: i32,
    pub max_lives: i32,

    // 无敌计时
    pub p1_invincible: f32,
    pub p2_invincible: f32,
    pub invincible_duration: f32,

    // 受伤减速
    pub p1_slow_timer: f32,
    pub p2_slow_timer: f32,
    pub slow_duration: f32,

    // 相机震动
    pub shake_timer: f32,
    pub shake_intensity: f32,

    // 游戏结束
    pub game_over: bool,
    pub game_over_timer: f32,
    pub winner_text: String,

    // 🎉 粒子系统
    pub particles: Vec<Particle>,

    // 得分
    pub p1_score: i32,
    pub p2_score: i32,
    // 浮动得分动画
    pub score_popups: Vec<ScorePopup>,

    // 长按 R 重置（游戏进行中）
    pub reset_hold_timer: f32,
    pub reset_hold_duration: f32,
    /// 是否已播放开场音效
    pub intro_played: bool,

    pub helper_window: helper_window::HelperWindow,

    pub items: items::Items,

    pub dbg_window: bool,

    // === 新增旋转无敌字段 ===
    pub p1_rotate_timer: f32,
    pub p2_rotate_timer: f32,
    pub rotate_duration: f32,
    pub rotate_speed: f32,

    // === 调试功能 ===
    pub debug_help_timer: f32,
    pub debug_help_layout: Option<atlas_text::TextLayout>,
}

impl AppContext {
    /// 玩家初始尺寸（更小，避免 P2 卡顿）
    const PLAYER_START_SIZE: f32 = 20.0;
    const PLAYER_SPAWN_FADE: f32 = 1.0;

    fn reset_player(player: &mut fish::Fish) {
        player.pos = Vec2::ZERO;
        player.size = Self::PLAYER_START_SIZE;
        player.alpha = 0.0;
        player.reset_color();
        player.eaten = false;
        player.spawn_fade = Self::PLAYER_SPAWN_FADE;
        player.spawn_fade_duration = Self::PLAYER_SPAWN_FADE;
    }

    fn reset_state(&mut self) {
        self.p1_lives = 5;
        self.p2_lives = 5;
        self.p1_invincible = 0.0;
        self.p2_invincible = 0.0;
        self.p1_slow_timer = 0.0;
        self.p2_slow_timer = 0.0;
        self.shake_timer = 0.0;
        self.shake_intensity = 0.0;
        self.camera.position = Vec2::ZERO;
        self.game_over = false;
        self.game_over_timer = 0.0;
        self.winner_text.clear();
        self.time_elapsed = 0.0;
        self.p1_score = 0;
        self.p2_score = 0;
        self.particles.clear();
        self.score_popups.clear();
        self.fishes = fishes::Fishes::new();
        self.reset_hold_timer = 0.0;
        self.intro_played = false;
        
        self.p1_rotate_timer = 0.0;
        self.p2_rotate_timer = 0.0;
    }

    fn restart(&mut self) {
        Self::reset_player(&mut self.player1_fish);
        Self::reset_player(&mut self.player2_fish);
        self.reset_state();

        // 重置物品系统
        self.items = items::Items::default();
        self.items.init_layouts(&mut self.atlas_text, &self.gfx);
        self.items.auto_spawn_enabled = true;
    }

    /// 🎉 在 pos 生成吃鱼粒子效果（被吃的鱼越大，粒子越大越多）
    fn spawn_eat_particles(&mut self, pos: Vec2, species: fish::FishSpecies, fish_size: f32) {
        let colors = species.bitten_colors();
        let size_scale = (fish_size / 20.0).max(0.5).min(5.0); // 参考 20 为基准
        let count = (15.0 + size_scale * 10.0) as usize;
        for _ in 0..count.min(60) {
            let ci = (fastrand::f32() * colors.len() as f32) as usize;
            let c = colors[ci.min(colors.len() - 1)];
            let angle = fastrand::f32() * 6.28;
            let speed = (30.0 + fastrand::f32() * 200.0) * size_scale.sqrt();
            let sz = (4.0 + fastrand::f32() * 14.0) * size_scale.sqrt();
            self.particles.push(Particle {
                pos,
                vel: vecf!(angle.cos() * speed, angle.sin() * speed),
                lifetime: 0.4 + fastrand::f32() * 0.8,
                max_lifetime: 1.2,
                size: sz,
                start_size: sz,
                color: [c[0], c[1], c[2], 1.0],
            });
        }
    }

    /// 更新粒子系统（粒子随时间缩小）
    fn update_particles(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.update(dt);
            let life_ratio = (p.lifetime / p.max_lifetime).max(0.0);
            p.size = p.start_size * life_ratio;
        }
        self.particles.retain(|p| p.alive());
        for popup in &mut self.score_popups {
            popup.lifetime -= dt;
            popup.pos.y += 30.0 * dt;
        }
        self.score_popups.retain(|p| p.lifetime > 0.0);
    }

    /// 处理单个玩家与所有物品的碰撞
    /// - `player_idx`: 0 表示 P1，1 表示 P2
    fn handle_player_item_collision(&mut self, player_idx: usize) {
        let (player_fish, other_fish, lives) = if player_idx == 0 {
            (
                &mut self.player1_fish,
                &mut self.player2_fish,
                &mut self.p1_lives,
            )
        } else {
            (
                &mut self.player2_fish,
                &mut self.player1_fish,
                &mut self.p2_lives,
            )
        };

        if *lives <= 0 {
            return;
        }

        let audio_mgr = &mut self.audio;
        let sounds = &self.sounds;
        let snd_item = sounds.get("snd_item").unwrap();

        let player_radius = player_fish.size * 0.6;
        self.items.foreach_overlap(
            player_fish.pos,
            player_radius,
            |_pos, _alpha, _mov, _size, item_type, already_touched| {
                if already_touched {
                    return true; // 已标记，不重复触发
                }
                match item_type {
                    items::Item::SizeToLife => {
                        // 尺寸减少，生命增加
                        player_fish.size = (player_fish.size * 0.8).max(6.0);
                        *lives = (*lives + 1).min(self.max_lives);
                        unsafe { audio_mgr.play(snd_item.clone()).unwrap_unchecked() };
                        true
                    }
                    items::Item::SizeSwap => {
                        // 交换两个玩家的大小
                        let temp = player_fish.size;
                        player_fish.size = other_fish.size;
                        other_fish.size = temp;
                        player_fish.size = player_fish.size.clamp(4.0, player_fish.max_size);
                        other_fish.size = other_fish.size.clamp(4.0, other_fish.max_size);
                        unsafe { audio_mgr.play(snd_item.clone()).unwrap_unchecked() };
                        true
                    }
                    // === 新增：旋转无敌道具 ===
                    items::Item::InvincibleRotate => {
                        if player_idx == 0 {
                            self.p1_rotate_timer = self.rotate_duration;
                        } else {
                            self.p2_rotate_timer = self.rotate_duration;
                        }
                        unsafe { audio_mgr.play(snd_item.clone()).unwrap_unchecked() };
                        true
                    }
                }
            },
        );
    }
}

// ─── free functions ─────────────────────────────────────────

impl Default for App {
    fn default() -> Self {
        App { ctx: None }
    }
}

fn fish_proc_move(
    player1_fish: &mut fish::Fish,
    player2_fish: &mut fish::Fish,
    driver: &EventDriver,
    dt: f32,
    p1_rotate_timer: f32,
    p2_rotate_timer: f32,
    p1_mult: f32,
    p2_mult: f32,
) {
    let v = 200.0;
    
    // P2 移动（方向键）
    let mut p2_dx = 0.0;
    let mut p2_dy = 0.0;
    if driver.keyboard().get_key_state(KeyCode::ArrowRight).is_pressed() { p2_dx += 1.0; }
    if driver.keyboard().get_key_state(KeyCode::ArrowLeft).is_pressed()  { p2_dx -= 1.0; }
    if driver.keyboard().get_key_state(KeyCode::ArrowDown).is_pressed()  { p2_dy += 1.0; }
    if driver.keyboard().get_key_state(KeyCode::ArrowUp).is_pressed()    { p2_dy -= 1.0; }
    
    if p2_dx != 0.0 || p2_dy != 0.0 {
        let p2_input = vecf!(p2_dx, p2_dy).normalize_or_zero();
        let p2_move = if p2_rotate_timer > 0.0 {
            player2_fish.rotate_input(p2_input.x, p2_input.y) * p2_mult
        } else {
            p2_input
        };
        player2_fish.pos += p2_move * v * dt;
        if p2_move.x > 0.0 { player2_fish.facing = fish::FishFacing::Right; }
        else if p2_move.x < 0.0 { player2_fish.facing = fish::FishFacing::Left; }
    }
    
    // P1 移动（WASD）
    let mut p1_dx = 0.0;
    let mut p1_dy = 0.0;
    if driver.keyboard().get_key_state(KeyCode::KeyD).is_pressed() { p1_dx += 1.0; }
    if driver.keyboard().get_key_state(KeyCode::KeyA).is_pressed() { p1_dx -= 1.0; }
    if driver.keyboard().get_key_state(KeyCode::KeyS).is_pressed() { p1_dy += 1.0; }
    if driver.keyboard().get_key_state(KeyCode::KeyW).is_pressed() { p1_dy -= 1.0; }
    
    if p1_dx != 0.0 || p1_dy != 0.0 {
        let p1_input = vecf!(p1_dx, p1_dy).normalize_or_zero();
        let p1_move = if p1_rotate_timer > 0.0 {
            player1_fish.rotate_input(p1_input.x, p1_input.y) * p1_mult
        } else {
            p1_input
        };
        player1_fish.pos += p1_move * v * dt;
        if p1_move.x > 0.0 { player1_fish.facing = fish::FishFacing::Right; }
        else if p1_move.x < 0.0 { player1_fish.facing = fish::FishFacing::Left; }
    }
}

fn play_sound(ctx: &mut AppContext, name: &str) {
    if let Some(data) = ctx.sounds.get(name) {
        let _ = ctx.audio.play(data.clone());
    }
}

fn process_event(ctx: &mut AppContext, dt: f64) -> Result<()> {
    if ctx.game_over { return Ok(()); }

    let dt_f = dt as f32;

    let p1_mult: f32 = if ctx.p1_slow_timer > 0.0 { 0.8 } else { 1.0 };
    let p2_mult: f32 = if ctx.p2_slow_timer > 0.0 { 0.8 } else { 1.0 };

    // 更新玩家（淡入 + 位置）
    let view_w = ctx.camera.viewport_size.x;
    let view_h = ctx.camera.viewport_size.y;
    ctx.player1_fish.update(dt_f, view_w, view_h);
    ctx.player2_fish.update(dt_f, view_w, view_h);

    fish_proc_move(
        &mut ctx.player1_fish,
        &mut ctx.player2_fish,
        &ctx.driver,
        dt_f,
        ctx.p1_rotate_timer,
        ctx.p2_rotate_timer,
        p1_mult,
        p2_mult,
    );

    // 更新进度 = sum(两个玩家 size)
    ctx.fishes.progress_size = (ctx.player1_fish.size + ctx.player2_fish.size).max(ctx.fishes.progress_size);

    let size = ctx.window.inner_size();
    ctx.fishes.set_view_size(size.width as f32, size.height as f32);
    ctx.fishes.update(dt_f, &mut ctx.atlas_text, &ctx.gfx);

    // ── 玩家1 ──
    if ctx.p1_lives > 0 {
        let invincible = ctx.p1_rotate_timer > 0.0;
        let eat_multiplier = if invincible { 10.0 } else { 1.0 };
        let p1_result = ctx.fishes.check_interact_advanced(
            ctx.player1_fish.pos,
            ctx.player1_fish.size,
            invincible,
            eat_multiplier,
        );
        if p1_result.eaten_count > 0 {
            ctx.player1_fish.size = (ctx.player1_fish.size + p1_result.last_eaten_size / ctx.player1_fish.size).min(ctx.player1_fish.max_size);
            let gain = (p1_result.eaten_count as f32 * 10.0 * p1_result.last_eaten_size / ctx.player1_fish.size) as i32;
            ctx.p1_score += gain;
            let eaten_species = p1_result.last_eaten_species.unwrap_or(fish::FishSpecies::Normal);
            ctx.spawn_eat_particles(ctx.player1_fish.pos, eaten_species, p1_result.last_eaten_size);
            play_sound(ctx, "snd_chomp");
        }
        if p1_result.hit_by_big && ctx.p1_invincible <= 0.0 {
            ctx.p1_lives -= 1;
            ctx.p1_invincible = ctx.invincible_duration;
            ctx.p1_slow_timer = ctx.slow_duration;
            ctx.shake_timer += 0.5;
            ctx.shake_intensity = 32.0;
            ctx.player1_fish.apply_hurt_flash();
            play_sound(ctx, "snd_hurt");
            if ctx.p1_lives <= 0 { ctx.player1_fish.alpha = 0.3; }
        }
    }

    // ── 玩家2 ──
    if ctx.p2_lives > 0 {
        let invincible = ctx.p2_rotate_timer > 0.0;
        let eat_multiplier = if invincible { 10.0 } else { 1.0 };
        let p2_result = ctx.fishes.check_interact_advanced(
            ctx.player2_fish.pos,
            ctx.player2_fish.size,
            invincible,
            eat_multiplier,
        );
        if p2_result.eaten_count > 0 {
            ctx.player2_fish.size = (ctx.player2_fish.size + p2_result.last_eaten_size / ctx.player2_fish.size).min(ctx.player2_fish.max_size);
            let gain = (p2_result.eaten_count as f32 * 10.0 * p2_result.last_eaten_size / ctx.player2_fish.size) as i32;
            ctx.p2_score += gain;
            let eaten_species = p2_result.last_eaten_species.unwrap_or(fish::FishSpecies::Normal);
            ctx.spawn_eat_particles(ctx.player2_fish.pos, eaten_species, p2_result.last_eaten_size);
            play_sound(ctx, "snd_chomp");
        }
        if p2_result.hit_by_big && ctx.p2_invincible <= 0.0 {
            ctx.p2_lives -= 1;
            ctx.p2_invincible = ctx.invincible_duration;
            ctx.p2_slow_timer = ctx.slow_duration;
            ctx.shake_timer += 0.5;
            ctx.shake_intensity = 32.0;
            ctx.player2_fish.apply_hurt_flash();
            play_sound(ctx, "snd_hurt");
            if ctx.p2_lives <= 0 { ctx.player2_fish.alpha = 0.3; }
        }
    }

    // ── 物品系统更新 ──
    ctx.items.set_progress(ctx.fishes.progress_size);
    ctx.items.update_foreach(dt, view_w, view_h);

    // ── 玩家与物品碰撞（复用函数） ──
    ctx.handle_player_item_collision(0); // P1
    ctx.handle_player_item_collision(1); // P2

    // ── 无敌 + 减速 ──
    if ctx.p1_invincible > 0.0 {
        ctx.p1_invincible -= dt_f;
        ctx.player1_fish.set_invincible_flash(ctx.p1_invincible);
        if ctx.p1_invincible <= 0.0 && ctx.p1_lives > 0 { ctx.player1_fish.reset_color(); ctx.player1_fish.alpha = 1.0; }
    }
    if ctx.p2_invincible > 0.0 {
        ctx.p2_invincible -= dt_f;
        ctx.player2_fish.set_invincible_flash(ctx.p2_invincible);
        if ctx.p2_invincible <= 0.0 && ctx.p2_lives > 0 { ctx.player2_fish.reset_color(); ctx.player2_fish.alpha = 1.0; }
    }
    if ctx.p1_slow_timer > 0.0 { ctx.p1_slow_timer -= dt_f; }
    if ctx.p2_slow_timer > 0.0 { ctx.p2_slow_timer -= dt_f; }

    // === 新增：旋转无敌计时器 ===
    if ctx.p1_rotate_timer > 0.0 {
        ctx.p1_rotate_timer -= dt_f;
        ctx.player1_fish.rot += ctx.rotate_speed * dt_f;
        // 金色闪烁特效
        let blink = (ctx.p1_rotate_timer * 8.0) as i32 % 2 == 0;
        if blink {
            ctx.player1_fish.color = [1.0, 0.8, 0.0, 1.0];
        } else {
            ctx.player1_fish.color = [1.0, 1.0, 1.0, 1.0];
        }
        if ctx.p1_rotate_timer <= 0.0 {
            ctx.player1_fish.reset_color();
        }
    }
    if ctx.p2_rotate_timer > 0.0 {
        ctx.p2_rotate_timer -= dt_f;
        ctx.player2_fish.rot += ctx.rotate_speed * dt_f;
        let blink = (ctx.p2_rotate_timer * 8.0) as i32 % 2 == 0;
        if blink {
            ctx.player2_fish.color = [1.0, 0.8, 0.0, 1.0];
        } else {
            ctx.player2_fish.color = [1.0, 1.0, 1.0, 1.0];
        }
        if ctx.p2_rotate_timer <= 0.0 {
            ctx.player2_fish.reset_color();
        }
    }

    // 相机震动
    if ctx.shake_timer > 0.0 {
        ctx.shake_timer -= dt_f;
        let intensity = ctx.shake_intensity * (ctx.shake_timer / 0.3).max(0.0);
        let angle = fastrand::f32() * 6.28;
        ctx.camera.position = vecf!(angle.cos() * intensity, angle.sin() * intensity);
    } else {
        ctx.camera.position = Vec2::ZERO;
    }

    // 更新粒子
    ctx.update_particles(dt_f);

    // 检查游戏结束
    if ctx.p1_lives <= 0 && ctx.p2_lives <= 0 {
        ctx.game_over = true;
        ctx.winner_text = "💀 双方阵亡！💀".to_string();
        play_sound(ctx, "snd_fail");
    } else if ctx.p1_lives <= 0 {
        ctx.game_over = true;
        ctx.winner_text = "💀 P2 胜利！👑".to_string();
        play_sound(ctx, "snd_fail");
    } else if ctx.p2_lives <= 0 {
        ctx.game_over = true;
        ctx.winner_text = "👑 P1 胜利！💀".to_string();
        play_sound(ctx, "snd_fail");
    }

    Ok(())
}

fn render_dbg_hud(ctx: &mut AppContext) -> Result<()> {
    let half_w = ctx.camera.viewport_size.x * 0.5;
    let half_h = ctx.camera.viewport_size.y * 0.5;

    let content_plsize = format!("\
p1_size: {}
p2_size: {}
prog: {}\
", ctx.player1_fish.size, ctx.player2_fish.size, ctx.fishes.progress_size);
    let content_facc = ctx.fishes.dbg_info();
    let content_item = ctx.items.dbg_info();

    let content = vec![content_plsize, content_facc, content_item].join("\n");

    let metrics = Metrics::new(16.0, 20.0);
    let attrs = Attrs::new().family(cosmic_text::Family::Name("Sarasa Mono SC"));
    let content_layout = ctx.atlas_text.layout_text(&content, metrics, attrs, &ctx.gfx.device)?;
    let offset = 24.0;
    let margin = 8.0;

    let box_tl = vecf!(-half_w + offset, -half_h + offset);
    let txt_tl = vecf!(-half_w + offset + margin, -half_h + offset + margin);
    let box_sz = content_layout.content_size + 2.0 * margin;

    ctx.shape_batch.add_rect_no_uv(box_tl, box_sz, Vec2::ZERO, 0.0, [0.0, 0.0, 0.0, 0.5]);
    ctx.atlas_text.render_layout(&content_layout, txt_tl, Vec2::ZERO, Transform2D::IDENTITY, [1.0;4], 0.0, &mut ctx.sprite_buf);

    Ok(())
}

/// HUD — 使用 TextLayout::content_size + add_rect_no_uv origin_px 对齐
fn render_hud(ctx: &mut AppContext) -> Result<()> {
    let half_w = ctx.camera.viewport_size.x * 0.5;
    let half_h = ctx.camera.viewport_size.y * 0.5;

    let p1_lives_text: String = (0..ctx.p1_lives).map(|_| "❤️").collect::<Vec<_>>().join("");
    let p2_lives_text: String = (0..ctx.p2_lives).map(|_| "❤️").collect::<Vec<_>>().join("");

    let p1_display = if ctx.p1_lives <= 0 { "💀".to_string() } else { p1_lives_text };
    let p2_display = if ctx.p2_lives <= 0 { "💀".to_string() } else { p2_lives_text };

    // HUD 第一行：生命值
    let p1_full = format!("P1 {}", p1_display);
    let p2_full = format!("{} P2", p2_display);

    // HUD 第二行：分数
    let p1_score_text = format!("{}分", ctx.p1_score);
    let p2_score_text = format!("{}分", ctx.p2_score);

    let metrics = Metrics::new(24.0, 24.0);
    let attrs = Attrs::new();
    let small_metrics = Metrics::new(18.0, 18.0);

    let p1_layout = ctx.atlas_text.layout_text(&p1_full, metrics, attrs.clone(), &ctx.gfx.device).unwrap();
    let p2_layout = ctx.atlas_text.layout_text(&p2_full, metrics, attrs.clone(), &ctx.gfx.device).unwrap();
    let p1_s_layout = ctx.atlas_text.layout_text(&p1_score_text, small_metrics, attrs.clone(), &ctx.gfx.device).unwrap();
    let p2_s_layout = ctx.atlas_text.layout_text(&p2_score_text, small_metrics, attrs, &ctx.gfx.device).unwrap();

    let margin = 24.0;
    let y_offset = 24.0;

    // 所有 UI 元素从左上角对齐，用 content_size 定位
    let line1_y = half_h - margin - y_offset;
    let line2_y = line1_y - 24.0 - 4.0;

    // P1 行1: 左边界 pos = (-half_w+margin, line1_y), origin = (0,0) 即左上
    let p1_left = -half_w + margin;
    let p1_pos = vecf!(p1_left, line1_y);
    // P2 行1: 右边界 pos = (half_w-margin, line1_y), origin = (content_size.x, 0) 即右上
    let p2_right = half_w - margin;
    let p2_pos = vecf!(p2_right, line1_y);

    // P1 行2
    let p1_score_pos = vecf!(p1_left, line2_y);
    // P2 行2
    let p2_score_pos = vecf!(p2_right, line2_y);

    let bg_padding = 5.0;
    let bg_h = p1_layout.content_size.y + bg_padding * 2.0;
    let bg_h2 = p1_s_layout.content_size.y + bg_padding * 2.0;
    let bg_w1 = p1_layout.content_size.x + bg_padding * 2.0;
    let bg_w2 = p2_layout.content_size.x + bg_padding * 2.0;
    let bg_w1s = p1_s_layout.content_size.x + bg_padding * 2.0;
    let bg_w2s = p2_s_layout.content_size.x + bg_padding * 2.0;

    // P1 行1 背景（左上对齐 p1_pos - padding）
    ctx.shape_batch.add_rect_no_uv(p1_pos + vecf!(-bg_padding, -bg_padding), vecf!(bg_w1, bg_h), Vec2::ZERO, 0.0, [0.0, 0.0, 0.0, 0.6]);
    // P2 行1 背景（右上对齐 p2_pos - padding）
    ctx.shape_batch.add_rect_no_uv(p2_pos + vecf!(bg_padding, -bg_padding), vecf!(bg_w2, bg_h), vecf!(bg_w2, 0.0), 0.0, [0.0, 0.0, 0.0, 0.6]);
    // P1 行2 背景
    ctx.shape_batch.add_rect_no_uv(p1_score_pos + vecf!(-bg_padding, -bg_padding), vecf!(bg_w1s, bg_h2), Vec2::ZERO, 0.0, [0.0, 0.0, 0.0, 0.6]);
    // P2 行2 背景（右上对齐）
    ctx.shape_batch.add_rect_no_uv(p2_score_pos + vecf!(bg_padding, -bg_padding), vecf!(bg_w2s, bg_h2), vecf!(bg_w2s, 0.0), 0.0, [0.0, 0.0, 0.0, 0.6]);

    let text_color = [1.0, 1.0, 1.0, 1.0];
    // P1 文字：左上对齐
    ctx.atlas_text.render_layout(&p1_layout, p1_pos, Vec2::ZERO, Transform2D::IDENTITY, text_color, 1.0, &mut ctx.sprite_buf);
    ctx.atlas_text.render_layout(&p1_s_layout, p1_score_pos, Vec2::ZERO, Transform2D::IDENTITY, [1.0, 1.0, 0.0, 1.0], 1.0, &mut ctx.sprite_buf);
    // P2 文字：用 content_size 右对齐（右上角对齐）
    ctx.atlas_text.render_layout(&p2_layout, p2_pos, vecf!(p2_layout.content_size.x, 0.0), Transform2D::IDENTITY, text_color, 1.0, &mut ctx.sprite_buf);
    ctx.atlas_text.render_layout(&p2_s_layout, p2_score_pos, vecf!(p2_s_layout.content_size.x, 0.0), Transform2D::IDENTITY, [1.0, 1.0, 0.0, 1.0], 1.0, &mut ctx.sprite_buf);

    if ctx.dbg_window { 
        render_dbg_hud(ctx)?;
    }

    Ok(())
}

/// 游戏结束画面
fn render_game_over(ctx: &mut AppContext) -> Result<()> {
    let size_v = vecf!(ctx.camera.viewport_size.x, ctx.camera.viewport_size.y);
    ctx.shape_batch.add_rect_no_uv(vecf!(0.0, 0.0), size_v*2.0, size_v, 0.0, [0.0, 0.0, 0.0, 0.5]);

    // 最终分数 — 居中显示
    let final_text = format!("{}  |  P1: {}分  P2: {}分", ctx.winner_text, ctx.p1_score, ctx.p2_score);
    let win_metrics = Metrics::new(40.0, 40.0);
    let win_layout = ctx.atlas_text.layout_text(&final_text, win_metrics, Attrs::new(), &ctx.gfx.device).unwrap();
    let win_origin = win_layout.content_size * 0.5;
    ctx.atlas_text.render_layout(&win_layout, vecf!(0.0, 0.0), win_origin, Transform2D::IDENTITY, [1.0; 4], 2.0, &mut ctx.sprite_buf);

    let hint_metrics = Metrics::new(20.0, 20.0);
    let hint_layout = ctx.atlas_text.layout_text("按 R 或 Enter 重新开始", hint_metrics, Attrs::new(), &ctx.gfx.device).unwrap();
    let hint_origin = hint_layout.content_size * 0.5;
    ctx.atlas_text.render_layout(&hint_layout, vecf!(0.0, -50.0), hint_origin, Transform2D::IDENTITY, [1.0; 4], 2.0, &mut ctx.sprite_buf);

    Ok(())
}

/// 调试帮助窗口（仅在 RJW_DEBUG=1 时显示）
fn render_debug_help(ctx: &mut AppContext) -> Result<()> {
    let layout = ctx.debug_help_layout.as_ref().unwrap();
    let alpha = if ctx.debug_help_timer < 5.0 {
        ctx.debug_help_timer / 5.0
    } else {
        1.0
    };
    let vp = ctx.camera.viewport_size;
    let margin = 16.0;
    // 左下角对齐
    let pos = vecf!(-vp.x * 0.5 + margin, -vp.y * 0.5 + margin);
    let content_size = layout.content_size;
    let bg_padding = 6.0;
    let bg_pos = pos + vecf!(-bg_padding, -bg_padding);
    let bg_size = content_size + Vec2::splat(bg_padding) * 2.0;

    ctx.shape_batch.add_rect_no_uv(bg_pos, bg_size, Vec2::ZERO, 0.0, [0.0, 0.0, 0.0, 0.7 * alpha]);
    ctx.atlas_text.render_layout(
        layout,
        Vec2::ZERO,
        Vec2::ZERO,
        Transform2D::IDENTITY.with_move_by(pos),
        [1.0, 1.0, 1.0, alpha],
        -1.0,
        &mut ctx.sprite_buf,
    );
    Ok(())
}

fn render_frame(ctx: &mut AppContext, dt: f64) -> Result<()> {
    ctx.gfx.clear_screen(&[0.1, 0.2, 0.4, 1.0]);

    ctx.camera.apply_viewport(&ctx.gfx);
    let mvp = ctx.camera.vp_matrix().transpose();

    unsafe {
        ctx.gfx.imm_context.OMSetBlendState(&ctx.gfx.states.blend_alpha, None, 0xFFFFFFFF);
        ctx.gfx.imm_context.RSSetState(&ctx.gfx.states.rasterizer_solid_cull_none);
        ctx.gfx.imm_context.OMSetDepthStencilState(&ctx.gfx.states.depth_none, 0);
        ctx.gfx.imm_context.PSSetSamplers(0, Some(&[Some(ctx.gfx.states.sampler_linear_clamp.clone())]));
    }

    // 网格
    grid_render::build_grid(&mut ctx.shape_batch, &ctx.camera, Vec2::splat(100.0), [0.1, 0.1, 0.15, 1.0]);
    ctx.shape_batch.set_mvp(&ctx.gfx, &mvp);
    ctx.shape_batch.submit_and_draw(&ctx.gfx)?;
    ctx.shape_batch.clear_batch();

    // 粒子（在鱼下方？正方？用 shape_batch 画圆）
    for p in &ctx.particles {
        let a = p.alpha();
        ctx.shape_batch.add_circle_no_uv(p.pos, p.size, [p.color[0], p.color[1], p.color[2], a * 0.7], 16);
    }
    ctx.shape_batch.set_mvp(&ctx.gfx, &mvp);
    ctx.shape_batch.submit_and_draw(&ctx.gfx)?;
    ctx.shape_batch.clear_batch();

    // 玩家 + 鱼群 + 物品
    ctx.player1_fish.add_to_buffer(&ctx.gfx, &mut ctx.atlas_text, &mut ctx.sprite_buf);
    ctx.player2_fish.add_to_buffer(&ctx.gfx, &mut ctx.atlas_text, &mut ctx.sprite_buf);
    ctx.fishes.add_to_buffer(&ctx.gfx, &mut ctx.atlas_text, &mut ctx.sprite_buf);
    ctx.items.draw_all(&mut ctx.atlas_text, &mut ctx.sprite_buf);
    
    // 注意：先 upload 再 draw_buffer_and_clear
    ctx.atlas_text.upload(&ctx.gfx)?;
    ctx.sprite_batch.set_mvp(&ctx.gfx, &mvp);
    ctx.sprite_batch.draw_buffer_and_clear(&ctx.gfx, &mut ctx.sprite_buf, |xform| (xform.pos, xform.scale, xform.rot));

    // HUD
    render_hud(ctx)?;
    ctx.shape_batch.set_mvp(&ctx.gfx, &mvp);
    ctx.shape_batch.submit_and_draw(&ctx.gfx)?;
    ctx.shape_batch.clear_batch();

    // 长按 R 进度条（左上角）
    if ctx.reset_hold_timer > 0.0 {
        let half_w = ctx.camera.viewport_size.x * 0.5;
        let half_h = ctx.camera.viewport_size.y * 0.5;
        let progress = (ctx.reset_hold_timer / ctx.reset_hold_duration).min(1.0);
        let bar_w = 200.0;
        let bar_h = 16.0;
        let margin = 10.0;
        let y_offset = 8.0;
        // 屏幕左上角（origin_px=ZERO = 左上）
        let bar_pos = vecf!(-half_w + margin, -half_h + y_offset);
        ctx.shape_batch.add_rect_no_uv(bar_pos, vecf!(bar_w, bar_h), Vec2::ZERO, 0.0, [0.0, 0.0, 0.0, 0.6]);
        ctx.shape_batch.add_rect_no_uv(bar_pos + vecf!(2.0, 2.0), vecf!((bar_w - 4.0) * progress, bar_h - 4.0), Vec2::ZERO, 0.0, [0.8, 0.2, 0.2, 0.9]);
        let hint = "松开取消重置".to_string();
        let hint_metrics = Metrics::new(14.0, 14.0);
        let hint_layout = ctx.atlas_text.layout_text(&hint, hint_metrics, Attrs::new(), &ctx.gfx.device).unwrap();
        ctx.atlas_text.render_layout(&hint_layout, bar_pos + vecf!(0.0, bar_h + 2.0), Vec2::ZERO, Transform2D::IDENTITY, [1.0, 1.0, 1.0, 1.0], 2.0, &mut ctx.sprite_buf);
    }

    if ctx.game_over { render_game_over(ctx)?; }

    ctx.shape_batch.set_mvp(&ctx.gfx, &mvp);
    ctx.shape_batch.submit_and_draw(&ctx.gfx)?;
    ctx.shape_batch.clear_batch();

    // 文字精灵可能已经在 render_hud 中添加到 sprite_buf，需要再次提交
    ctx.atlas_text.upload(&ctx.gfx)?;
    ctx.sprite_batch.set_mvp(&ctx.gfx, &mvp);
    ctx.sprite_batch.draw_buffer_and_clear(&ctx.gfx, &mut ctx.sprite_buf, |xform| (xform.pos, xform.scale, xform.rot));

    ctx.helper_window.render(dt as f32, ctx.camera.viewport_size, &mut ctx.atlas_text, &mut ctx.sprite_buf, &mut ctx.sprite_batch, &mut ctx.shape_batch, &ctx.gfx)?;

    // 调试帮助窗口（仅在调试模式下显示）
    if ctx.dbg_window && ctx.debug_help_timer > 0.0 {
        render_debug_help(ctx)?;
        ctx.shape_batch.set_mvp(&ctx.gfx, &mvp);
        ctx.shape_batch.submit_and_draw(&ctx.gfx)?;
        ctx.shape_batch.clear_batch();
        ctx.atlas_text.upload(&ctx.gfx)?;
        ctx.sprite_batch.set_mvp(&ctx.gfx, &mvp);
        ctx.sprite_batch.draw_buffer_and_clear(&ctx.gfx, &mut ctx.sprite_buf, |xform| (xform.pos, xform.scale, xform.rot));
    }

    ctx.gfx.present()?;
    Ok(())
}

impl App {
    fn init_audio(&mut self) -> Result<(kira::AudioManager, HashMap<String, StaticSoundData>)> {
        let mut sounds = HashMap::new();
        macro_rules! insert_snd {
            ($name:expr, $dir:expr) => { sounds.insert($name.to_string(), StaticSoundData::from_cursor(Cursor::new(include_bytes!($dir)))?); };
        }
        insert_snd!("snd_ominous", "snd_ominous.wav");
        insert_snd!("snd_hurt", "snd_hurt.wav");
        insert_snd!("snd_chomp", "snd_chomp.wav");
        insert_snd!("snd_fail", "snd_badexplosion.wav");
        insert_snd!("snd_item", "snd_kikkyspace.wav");
        Ok((kira::AudioManager::new(Default::default()).context("AudioManager::new failed")?, sounds))
    }

    /// 开局预加载所有可能的字形，避免游戏过程中动态 rasterize
    fn preload_glyphs(atlas_text: &mut AtlasText, gfx: &graphic::d3d11::D3D11) -> Result<()> {
        use fish::FishSpecies;
        use krjw_engine::cosmic_text::{Attrs, Metrics};

        // 预加载所有鱼种的 Emoji（用各自的 max_size）
        const ALL_SPECIES: &[FishSpecies] = &[
            FishSpecies::Normal, FishSpecies::Tropical, FishSpecies::Puffer, FishSpecies::Octopus,
            FishSpecies::Whale, FishSpecies::Shark, FishSpecies::Dolphin, FishSpecies::Crab,
            FishSpecies::Lobster, FishSpecies::Turtle, FishSpecies::WaterHawk,
        ];
        for &species in ALL_SPECIES {
            let (_, max_s) = species.size_range();
            let emoji = species.emoji();
            let _ = atlas_text.layout_text(emoji, Metrics::new(max_s * 2.0, max_s * 2.0), Attrs::new(), &gfx.device)?;
        }

        // 预加载玩家鱼 Emoji（最大尺寸 256）
        let _ = atlas_text.layout_text("🐠", Metrics::new(512.0, 512.0), Attrs::new(), &gfx.device)?;
        let _ = atlas_text.layout_text("🐟", Metrics::new(512.0, 512.0), Attrs::new(), &gfx.device)?;

        // 预加载物品 Emoji
        for item in items::Item::ALL {
            let emoji = item.emoji();
            let render_size = item.max_render_size() * 2.0;
            let _ = atlas_text.layout_text(emoji, Metrics::new(render_size, render_size), Attrs::new(), &gfx.device)?;
        }

        // 预加载 HUD 常用字符
        let _ = atlas_text.layout_text("❤️💀P120分按R或Enter重新开始双方阵亡胜利🧾🩸", Metrics::new(48.0, 48.0), Attrs::new(), &gfx.device)?;
        let _ = atlas_text.layout_text("❤️💀P120分", Metrics::new(24.0, 24.0), Attrs::new(), &gfx.device)?;

        Ok(())
    }

    pub fn run(&mut self, window: winit::window::Window, hwnd: isize, rx: Receiver<AppMsg>) -> Result<()> {
        window.set_transparent(true);

        let gfx = D3D11::init_on_hwnd(hwnd)?;
        let size = window.inner_size();

        let driver = EventDriver::new(rx, &window);

        let sprite_batch = SpriteBatch2D::new(&gfx.device, 2048, &gfx.states.vs_puc_m_2d, &gfx.states.ps_tex_rgba_2d, &gfx.states.input_layout_puc)?;
        let shape_batch = ShapeBatch2D::new(&gfx.device, 4096, &gfx.states.vs_puc_m_2d, &gfx.states.ps_solid_2d, &gfx.states.input_layout_puc)?;

        let camera = Camera2D::new(vecf!(size.width as f32, size.height as f32));
        let timer = Timer::default();
        let sprite_buf = Sprite2DBuffer::default();
        let mut atlas_text = AtlasText::new(&gfx.device, -50.0, 12000.0).context("AtlasText::new failed")?;

        // 开局预加载所有字形
        Self::preload_glyphs(&mut atlas_text, &gfx)?;

        // 玩家鱼（统一使用 PLAYER_START_SIZE）
        let player1_fish = fish::Fish::new(AppContext::PLAYER_START_SIZE, 256.0, "🐠", &mut atlas_text, &gfx);
        let player2_fish = fish::Fish::new(AppContext::PLAYER_START_SIZE, 256.0, "🐟", &mut atlas_text, &gfx);

        let fishes = fishes::Fishes::new();

        let (audio, sounds) = self.init_audio()?;

        let helper_window = helper_window::HelperWindow::new(&mut atlas_text, &gfx)?;

        let mut items = items::Items::default();
        items.init_layouts(&mut atlas_text, &gfx);
        // 可选：设置自动生成
        items.auto_spawn_enabled = true;

        let dbg_window = if Ok("1".to_string()) == std::env::var("RJW_DEBUG") { true } else { false };

        let mut ctx = AppContext {
            window, driver, gfx, sprite_batch, shape_batch, camera, timer, sprite_buf, atlas_text,
            time_elapsed: 0.0, audio, sounds,
            player1_fish, player2_fish, fishes,
            p1_lives: 5, p2_lives: 5, max_lives: 10,
            p1_invincible: 0.0, p2_invincible: 0.0, invincible_duration: 1.5,
            p1_slow_timer: 0.0, p2_slow_timer: 0.0, slow_duration: 1.0,
            shake_timer: 0.0, shake_intensity: 0.0,
            game_over: false, game_over_timer: 0.0, winner_text: String::new(),
            particles: Vec::with_capacity(1024),
            p1_score: 0, p2_score: 0,
            score_popups: Vec::new(),
            reset_hold_timer: 0.0,
            reset_hold_duration: 2.0,
            intro_played: false,
            helper_window,
            items,
            dbg_window,
            p1_rotate_timer: 0.0,
            p2_rotate_timer: 0.0,
            rotate_duration: 15.0,
            rotate_speed: 4.0 * std::f32::consts::PI,  // 每秒2圈
            debug_help_timer: 10.0,
            debug_help_layout: None,
        };

        // 初始化调试帮助布局（仅在调试模式下）
        if ctx.dbg_window {
            let metrics = Metrics::new(16.0, 20.0);
            let attrs = Attrs::new().family(cosmic_text::Family::Name("SimHei"));
            let layout = ctx.atlas_text.layout_text(
                include_str!("app/dbg_help.txt"),
                metrics,
                attrs,
                &ctx.gfx.device,
            )?;
            ctx.debug_help_layout = Some(layout);
        }

        ctx.restart();
        self.ctx = Some(ctx);

        loop {
            let ctx = self.ctx.as_mut().unwrap();
            let events = ctx.driver.poll_frame();
            if events.close_requested || events.disconnected { break; }

            let size = ctx.window.inner_size();
            let size = (size.width, size.height);
            let dt = ctx.timer.pre_frame_and_get_delta_time();
            let dt_f = dt as f32;

            // 开场音效
            if !ctx.intro_played {
                play_sound(ctx, "snd_ominous");
                ctx.intro_played = true;
            }

            // 长按 R 5 秒重置（游戏进行中）
            let ks_r = ctx.driver.keyboard().get_key_state(KeyCode::KeyR);
            if !ctx.game_over && ks_r.is_pressed() {
                ctx.reset_hold_timer += dt_f;
                if ctx.reset_hold_timer >= ctx.reset_hold_duration {
                    ctx.restart();
                }
            } else {
                ctx.reset_hold_timer = 0.0;
            }

            // game_over 时重新开始
            if ctx.game_over {
                let ks_ent = ctx.driver.keyboard().get_key_state(KeyCode::Enter);
                if ks_r.is_down_true_edge() || ks_ent.is_down_true_edge() { ctx.restart(); }
            }

            // === 调试功能：按下 1 生成随机道具，按下 2 生成随机鱼 ===
            if ctx.dbg_window {
                let ks1 = ctx.driver.keyboard().get_key_state(KeyCode::Digit1);
                if ks1.is_down_true_edge() {
                    let item = items::Item::random_weighted();
                    let view_w = ctx.camera.viewport_size.x;
                    let view_h = ctx.camera.viewport_size.y;
                    ctx.items.new_item(item, view_w, view_h);
                    ctx.debug_help_timer = 10.0;
                }

                let ks2 = ctx.driver.keyboard().get_key_state(KeyCode::Digit2);
                if ks2.is_down_true_edge() {
                    if let Some(&species) = fastrand::choice(ALL_FISH_SPECIES) {
                        ctx.fishes.spawn_one_of_species(species, &mut ctx.atlas_text, &ctx.gfx);
                        ctx.debug_help_timer = 10.0;
                    }
                }
            }

            // 更新调试帮助计时器
            if ctx.debug_help_timer > 0.0 {
                ctx.debug_help_timer -= dt_f;
            }

            ctx.driver.if_window_size_dirty(|w, h| { ctx.gfx.on_resize(w, h)?; ctx.camera.viewport_size = vecf!(w as f32, h as f32); Ok(()) })?;

            process_event(ctx, dt)?;

            if size.0 > 0 && size.1 > 0 { render_frame(ctx, dt)?; }
            ctx.driver.end_frame();
            ctx.timer.post_frame_fpsc(dt);
            ctx.time_elapsed += dt;
        }
        Ok(())
    }
}