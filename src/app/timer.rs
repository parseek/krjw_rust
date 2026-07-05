use std::time;

#[derive(Debug)]
pub struct Timer {
    frame_stamp: time::Instant,
    fpsc_frame_stamp: time::Instant,
    fpsc_duration_acc: f64,
    fpsc_frame_count: u64,
    fps: f64,
}

impl Default for Timer {
    fn default() -> Self {
        Self {
            frame_stamp: time::Instant::now(),
            fpsc_frame_stamp: time::Instant::now(),
            fpsc_duration_acc: 0.0,
            fpsc_frame_count: 0,
            fps: 0.0
        }
    }
}

impl Timer {
    #[allow(unused)]
    pub fn get_fps(&self) -> f64 {
        self.fps
    }
    pub fn pre_frame_and_get_delta_time(&mut self) -> f64 {
        let frame_stamp = time::Instant::now();
        let delta_time = (frame_stamp - self.frame_stamp).as_secs_f64();
        self.frame_stamp = frame_stamp;
        delta_time
    }
    /// Used to calculate the FPS.
    #[allow(unused)]
    pub fn post_frame_fpsc(&mut self) {
        let frame_stamp = time::Instant::now();
        let delta_time = (frame_stamp - self.frame_stamp).as_secs_f64();
        self.fpsc_frame_stamp = frame_stamp;
        self.fpsc_duration_acc += delta_time;
        self.fpsc_frame_count += 1;

        if self.fpsc_duration_acc > 0.99 {
            self.fps = self.fpsc_frame_count as f64 / self.fpsc_duration_acc;
            self.fpsc_frame_count = 0;
            self.fpsc_duration_acc = 0.0;
        }
    }
}