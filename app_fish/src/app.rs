use anyhow::{Context, Result};
use glam::Vec2;
use kira::sound::static_sound::StaticSoundData;
use krjw_engine::{cosmic_text::{Attrs, Metrics}, winit::keyboard::KeyCode, *};
use std::{collections::HashMap, io::Cursor, sync::mpsc::Receiver};

mod grid_render;
mod fish;
mod fishes;

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
    text: String,
    lifetime: f32,
    max_lifetime: f32,
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

    /// 开局生成几条基础鱼
    fn spawn_starter_fish(&mut self) {
        let view_w = self.camera.viewport_size.x;
        let view_h = self.camera.viewport_size.y;
        self.fishes = fishes::Fishes::new(view_w, view_h);
        for _ in 0..4 {
            self.fishes.spawn_one_of_species(fish::FishSpecies::Normal, &mut self.atlas_text, &self.gfx);
            self.fishes.spawn_one_of_species(fish::FishSpecies::Tropical, &mut self.atlas_text, &self.gfx);
        }
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
        self.reset_hold_timer = 0.0;
        self.intro_played = false;
    }

    fn restart(&mut self) {
        Self::reset_player(&mut self.player1_fish);
        Self::reset_player(&mut self.player2_fish);
        self.reset_state();
        self.spawn_starter_fish();
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
                vel: Vec2::new(angle.cos() * speed, angle.sin() * speed),
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
}

// ─── free functions ─────────────────────────────────────────

impl Default for App {
    fn default() -> Self {
        App { ctx: None }
    }
}

fn fish_proc_move(player1_fish: &mut fish::Fish, player2_fish: &mut fish::Fish, driver: &EventDriver, dt: f32) {
    let v = 200.0;
    if driver.keyboard().get_key_state(KeyCode::ArrowRight).is_pressed() {
        player2_fish.pos.x += v * dt;
        player2_fish.facing = fish::FishFacing::Right;
    } else if driver.keyboard().get_key_state(KeyCode::ArrowLeft).is_pressed() {
        player2_fish.pos.x -= v * dt;
        player2_fish.facing = fish::FishFacing::Left;
    }
    if driver.keyboard().get_key_state(KeyCode::ArrowDown).is_pressed() {
        player2_fish.pos.y += v * dt;
    } else if driver.keyboard().get_key_state(KeyCode::ArrowUp).is_pressed() {
        player2_fish.pos.y -= v * dt;
    }
    if driver.keyboard().get_key_state(KeyCode::KeyD).is_pressed() {
        player1_fish.pos.x += v * dt;
        player1_fish.facing = fish::FishFacing::Right;
    } else if driver.keyboard().get_key_state(KeyCode::KeyA).is_pressed() {
        player1_fish.pos.x -= v * dt;
        player1_fish.facing = fish::FishFacing::Left;
    }
    if driver.keyboard().get_key_state(KeyCode::KeyS).is_pressed() {
        player1_fish.pos.y += v * dt;
    } else if driver.keyboard().get_key_state(KeyCode::KeyW).is_pressed() {
        player1_fish.pos.y -= v * dt;
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

    fish_proc_move(&mut ctx.player1_fish, &mut ctx.player2_fish, &ctx.driver, dt_f * p1_mult.min(p2_mult));

    // 更新进度 = max(两个玩家 size)
    ctx.fishes.progress_size = ctx.player1_fish.size.max(ctx.player2_fish.size);

    let size = ctx.window.inner_size();
    ctx.fishes.set_view_size(size.width as f32, size.height as f32);
    ctx.fishes.update(dt_f, &mut ctx.atlas_text, &ctx.gfx);

    // ── 玩家1 ──
    if ctx.p1_lives > 0 {
        let p1_result = ctx.fishes.check_interact(ctx.player1_fish.pos, ctx.player1_fish.size);
        if p1_result.eaten_count > 0 {
            ctx.player1_fish.size = (ctx.player1_fish.size + p1_result.eaten_count as f32 * 0.5).min(ctx.player1_fish.max_size);
            // 得分 + 粒子效果
            let gain = (p1_result.eaten_count as f32 * 10.0) as i32;
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
            ctx.shake_intensity = 20.0;
            ctx.player1_fish.apply_hurt_flash();
            play_sound(ctx, "snd_hurt");
            if ctx.p1_lives <= 0 { ctx.player1_fish.alpha = 0.3; }
        }
    }

    // ── 玩家2 ──
    if ctx.p2_lives > 0 {
        let p2_result = ctx.fishes.check_interact(ctx.player2_fish.pos, ctx.player2_fish.size);
        if p2_result.eaten_count > 0 {
            ctx.player2_fish.size = (ctx.player2_fish.size + p2_result.eaten_count as f32 * 0.5).min(ctx.player2_fish.max_size);
            let gain = (p2_result.eaten_count as f32 * 10.0) as i32;
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
            ctx.shake_intensity = 20.0;
            ctx.player2_fish.apply_hurt_flash();
            play_sound(ctx, "snd_hurt");
            if ctx.p2_lives <= 0 { ctx.player2_fish.alpha = 0.3; }
        }
    }

    // 无敌 + 减速
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

    // 相机震动
    if ctx.shake_timer > 0.0 {
        ctx.shake_timer -= dt_f;
        let intensity = ctx.shake_intensity * (ctx.shake_timer / 0.3).max(0.0);
        let angle = fastrand::f32() * 6.28;
        ctx.camera.position = Vec2::new(angle.cos() * intensity, angle.sin() * intensity);
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

/// HUD
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

    let margin = 10.0;
    let y_offset = 8.0;
    let line1_y = half_h - margin - 24.0 - y_offset;
    let line2_y = line1_y - 24.0 - 4.0; // 第二行在生命值下面

    let p1_pos = Vec2::new(-half_w + margin, line1_y);
    let p1_score_pos = Vec2::new(-half_w + margin, line2_y);

    let bg_padding = 4.0;
    let p1_bg_w = 20.0 + p1_display.len() as f32 * 16.0;
    let p2_bg_w = 20.0 + p2_display.len() as f32 * 16.0;
    let bg_h = 28.0;
    let bg_h2 = 24.0;

    // P1 背景
    ctx.shape_batch.add_rect_no_uv(p1_pos + Vec2::new(-bg_padding, -bg_padding), Vec2::new(p1_bg_w + bg_padding * 2.0, bg_h + bg_padding * 2.0), 0.0, [0.0, 0.0, 0.0, 0.6]);
    // P2 背景
    let p2_bg_x = half_w - margin - p2_bg_w - bg_padding;
    ctx.shape_batch.add_rect_no_uv(Vec2::new(p2_bg_x, p1_pos.y - bg_padding), Vec2::new(p2_bg_w + bg_padding * 2.0, bg_h + bg_padding * 2.0), 0.0, [0.0, 0.0, 0.0, 0.6]);
    // 分数背景
    let p1_sbg_w = p1_score_text.len() as f32 * 12.0 + 10.0;
    let p2_sbg_w = p2_score_text.len() as f32 * 12.0 + 10.0;
    ctx.shape_batch.add_rect_no_uv(p1_score_pos + Vec2::new(-bg_padding, -bg_padding), Vec2::new(p1_sbg_w + bg_padding * 2.0, bg_h2 + bg_padding * 2.0), 0.0, [0.0, 0.0, 0.0, 0.6]);
    let p2_sbg_x = half_w - margin - p2_sbg_w - bg_padding;
    ctx.shape_batch.add_rect_no_uv(Vec2::new(p2_sbg_x, p1_score_pos.y - bg_padding), Vec2::new(p2_sbg_w + bg_padding * 2.0, bg_h2 + bg_padding * 2.0), 0.0, [0.0, 0.0, 0.0, 0.6]);

    let text_color = [1.0, 1.0, 1.0, 1.0];
    ctx.atlas_text.render_layout(&p1_layout, p1_pos, Vec2::ZERO, Transform2D::IDENTITY, text_color, 1.0, &mut ctx.sprite_buf);
    ctx.atlas_text.render_layout(&p2_layout, Vec2::new(half_w - margin - p2_bg_w, p1_pos.y), Vec2::ZERO, Transform2D::IDENTITY, text_color, 1.0, &mut ctx.sprite_buf);
    ctx.atlas_text.render_layout(&p1_s_layout, p1_score_pos, Vec2::ZERO, Transform2D::IDENTITY, [1.0, 1.0, 0.0, 1.0], 1.0, &mut ctx.sprite_buf);
    ctx.atlas_text.render_layout(&p2_s_layout, Vec2::new(half_w - margin - p2_sbg_w, p1_score_pos.y), Vec2::ZERO, Transform2D::IDENTITY, [1.0, 1.0, 0.0, 1.0], 1.0, &mut ctx.sprite_buf);

    Ok(())
}

/// 游戏结束画面
fn render_game_over(ctx: &mut AppContext) -> Result<()> {
    let half_w = ctx.camera.viewport_size.x * 0.5;
    let half_h = ctx.camera.viewport_size.y * 0.5;

    ctx.shape_batch.add_rect_no_uv(Vec2::new(-half_w, -half_h), Vec2::new(ctx.camera.viewport_size.x, ctx.camera.viewport_size.y), 0.0, [0.0, 0.0, 0.0, 0.5]);

    // 最终分数
    let final_text = format!("{}  |  P1: {}分  P2: {}分", ctx.winner_text, ctx.p1_score, ctx.p2_score);
    let win_metrics = Metrics::new(40.0, 40.0);
    let win_layout = ctx.atlas_text.layout_text(&final_text, win_metrics, Attrs::new(), &ctx.gfx.device).unwrap();
    let win_pos = Vec2::new(-half_w + 30.0, 0.0);
    ctx.atlas_text.render_layout(&win_layout, win_pos, Vec2::ZERO, Transform2D::IDENTITY, [1.0, 1.0, 0.0, 1.0], 2.0, &mut ctx.sprite_buf);

    let hint_metrics = Metrics::new(20.0, 20.0);
    let hint_layout = ctx.atlas_text.layout_text("按 R 或 Enter 重新开始", hint_metrics, Attrs::new(), &ctx.gfx.device).unwrap();
    ctx.atlas_text.render_layout(&hint_layout, Vec2::new(-half_w + 30.0, -50.0), Vec2::ZERO, Transform2D::IDENTITY, [1.0, 1.0, 1.0, 1.0], 2.0, &mut ctx.sprite_buf);

    Ok(())
}

fn render_frame(ctx: &mut AppContext) -> Result<()> {
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

    // 玩家 + 鱼群
    ctx.player1_fish.add_to_buffer(&ctx.gfx, &mut ctx.atlas_text, &mut ctx.sprite_buf);
    ctx.player2_fish.add_to_buffer(&ctx.gfx, &mut ctx.atlas_text, &mut ctx.sprite_buf);
    ctx.fishes.add_to_buffer(&ctx.gfx, &mut ctx.atlas_text, &mut ctx.sprite_buf);
    
    ctx.sprite_batch.push_buffered(&ctx.gfx, &mvp, &mut ctx.sprite_buf, |xform| (xform.pos, xform.scale, xform.rot));
    ctx.sprite_batch.clear_batch();
    ctx.sprite_buf.clear();

    // HUD
    render_hud(ctx)?;
    ctx.shape_batch.set_mvp(&ctx.gfx, &mvp);
    ctx.shape_batch.submit_and_draw(&ctx.gfx)?;
    ctx.shape_batch.clear_batch();

    // 长按 R 进度条（左上角，与 HUD 对齐）
    if ctx.reset_hold_timer > 0.0 {
        let half_w = ctx.camera.viewport_size.x * 0.5;
        let half_h = ctx.camera.viewport_size.y * 0.5;
        let progress = (ctx.reset_hold_timer / ctx.reset_hold_duration).min(1.0);
        let bar_w = 200.0;
        let bar_h = 16.0;
        let margin = 10.0;
        let y_offset = 8.0;
        // 放在 HUD 第二行（分数行）的下方，靠左对齐
        let line3_y = - half_h + margin + 24.0 + y_offset + (24.0 + 4.0) + (24.0 + 4.0);
        let bar_pos = Vec2::new(-half_w + margin, line3_y);
        ctx.shape_batch.add_rect_no_uv(bar_pos, Vec2::new(bar_w, bar_h), 0.0, [0.0, 0.0, 0.0, 0.6]);
        ctx.shape_batch.add_rect_no_uv(bar_pos + Vec2::new(2.0, 2.0), Vec2::new((bar_w - 4.0) * progress, bar_h - 4.0), 0.0, [0.8, 0.2, 0.2, 0.9]);
        let hint = format!("松开取消重置");
        let hint_metrics = Metrics::new(14.0, 14.0);
        let hint_layout = ctx.atlas_text.layout_text(&hint, hint_metrics, Attrs::new(), &ctx.gfx.device).unwrap();
        ctx.atlas_text.render_layout(&hint_layout, Vec2::new(-half_w + margin + 4.0, line3_y + 1.0), Vec2::ZERO, Transform2D::IDENTITY, [1.0, 1.0, 1.0, 1.0], 2.0, &mut ctx.sprite_buf);
    }

    if ctx.game_over { render_game_over(ctx)?; }

    ctx.shape_batch.set_mvp(&ctx.gfx, &mvp);
    ctx.shape_batch.submit_and_draw(&ctx.gfx)?;
    ctx.shape_batch.clear_batch();

    ctx.atlas_text.upload(&ctx.gfx)?;
    ctx.sprite_batch.push_buffered(&ctx.gfx, &mvp, &mut ctx.sprite_buf, |xform| (xform.pos, xform.scale, xform.rot));
    ctx.sprite_batch.clear_batch();
    ctx.sprite_buf.clear();

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

        // 预加载 HUD 常用字符
        let _ = atlas_text.layout_text("❤️💀P120分按R或Enter重新开始双方阵亡胜利", Metrics::new(48.0, 48.0), Attrs::new(), &gfx.device)?;
        let _ = atlas_text.layout_text("❤️💀P120分", Metrics::new(24.0, 24.0), Attrs::new(), &gfx.device)?;

        Ok(())
    }

    pub fn run(&mut self, window: winit::window::Window, hwnd: isize, rx: Receiver<AppMsg>) -> Result<()> {
        window.set_transparent(true);

        let gfx = D3D11::init_on_hwnd(hwnd)?;
        let size = window.inner_size();

        let mut driver = EventDriver::new(rx);
        driver.set_initial_window_size(size.width, size.height);

        let sprite_batch = SpriteBatch2D::new(&gfx.device, 2048, &gfx.states.vs_puc_m_2d, &gfx.states.ps_tex_rgba_2d, &gfx.states.input_layout_puc)?;
        let shape_batch = ShapeBatch2D::new(&gfx.device, 4096, &gfx.states.vs_puc_m_2d, &gfx.states.ps_solid_2d, &gfx.states.input_layout_puc)?;

        let camera = Camera2D::new(Vec2::new(size.width as f32, size.height as f32));
        let timer = Timer::default();
        let sprite_buf = Sprite2DBuffer::default();
        let mut atlas_text = AtlasText::new(&gfx.device, -50.0, 12000.0).context("AtlasText::new failed")?;

        // 开局预加载所有字形
        Self::preload_glyphs(&mut atlas_text, &gfx)?;

        // 玩家鱼（统一使用 PLAYER_START_SIZE）
        let player1_fish = fish::Fish::new(AppContext::PLAYER_START_SIZE, 256.0, "🐠", &mut atlas_text, &gfx);
        let player2_fish = fish::Fish::new(AppContext::PLAYER_START_SIZE, 256.0, "🐟", &mut atlas_text, &gfx);

        let mut fishes = fishes::Fishes::new(size.width as f32, size.height as f32);
        for _ in 0..4 {
            fishes.spawn_one_of_species(fish::FishSpecies::Normal, &mut atlas_text, &gfx);
            fishes.spawn_one_of_species(fish::FishSpecies::Tropical, &mut atlas_text, &gfx);
        }

        let (audio, sounds) = self.init_audio()?;

        self.ctx = Some(AppContext {
            window, driver, gfx, sprite_batch, shape_batch, camera, timer, sprite_buf, atlas_text,
            time_elapsed: 0.0, audio, sounds,
            player1_fish, player2_fish, fishes,
            p1_lives: 5, p2_lives: 5, max_lives: 5,
            p1_invincible: 0.0, p2_invincible: 0.0, invincible_duration: 1.5,
            p1_slow_timer: 0.0, p2_slow_timer: 0.0, slow_duration: 1.0,
            shake_timer: 0.0, shake_intensity: 0.0,
            game_over: false, game_over_timer: 0.0, winner_text: String::new(),
            particles: Vec::new(),
            p1_score: 0, p2_score: 0,
            score_popups: Vec::new(),
            reset_hold_timer: 0.0,
            reset_hold_duration: 5.0,
            intro_played: false,
        });

        loop {
            let ctx = self.ctx.as_mut().unwrap();
            let events = ctx.driver.poll_frame();
            if events.close_requested || events.disconnected { break; }

            let size = ctx.window.inner_size();
            let size = (size.width, size.height);
            let dt = ctx.timer.pre_frame_and_get_delta_time();

            // 开场音效
            if !ctx.intro_played {
                play_sound(ctx, "snd_ominous");
                ctx.intro_played = true;
            }

            // 长按 R 5 秒重置（游戏进行中）
            let dt_f = dt as f32;
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

            ctx.driver.if_window_size_dirty(|w, h| { ctx.gfx.on_resize(w, h)?; ctx.camera.viewport_size = Vec2::new(w as f32, h as f32); Ok(()) })?;

            process_event(ctx, dt)?;

            if size.0 > 0 && size.1 > 0 { render_frame(ctx)?; }
            ctx.driver.end_frame();
            ctx.timer.post_frame_fpsc(dt);
            ctx.time_elapsed += dt;
        }
        Ok(())
    }
}