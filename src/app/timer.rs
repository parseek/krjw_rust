use std::time;

#[derive(Debug)]
pub struct Timer {
    frame_stamp: time::Instant,
    fps: f64,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            frame_stamp: time::Instant::now(),
            fps: 0.0,
        }
    }
}

impl Timer {
    /// Current smoothed FPS value (updated every frame via EMA).
    /// 当前平滑后的 FPS 值（每帧通过 EMA 更新）。
    pub fn get_fps(&self) -> f64 {
        self.fps
    }

    /// Compute delta time since last call and advance the frame stamp.
    /// 计算距上次调用的帧间隔，并更新帧时间戳。
    pub fn pre_frame_and_get_delta_time(&mut self) -> f64 {
        let now = time::Instant::now();
        let dt = (now - self.frame_stamp).as_secs_f64();
        self.frame_stamp = now;
        dt
    }

    /// Update the smoothed FPS using exponential moving average (EMA).
    /// 使用指数移动平均（EMA）更新平滑 FPS。
    ///
    /// Takes the same `dt` that was returned by `pre_frame_and_get_delta_time`
    /// (possibly clamped) so no extra `Instant::now()` call is needed.
    /// 使用与 `pre_frame_and_get_delta_time` 相同的 `dt`（可能已被 clamp），
    /// 避免重复取时间戳。
    ///
    /// The EMA constant α = 0.1 provides a good balance between
    /// responsiveness and smoothness.
    /// EMA 常数 α = 0.1，在响应速度和平滑度之间取得良好平衡。
    pub fn post_frame_fpsc(&mut self, dt: f64) {
        let alpha = 0.1;
        let instant_fps = 1.0 / dt.max(1e-10);
        self.fps = self.fps * (1.0 - alpha) + instant_fps * alpha;
    }
}