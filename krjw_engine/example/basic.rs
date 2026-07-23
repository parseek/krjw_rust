use anyhow::{Context, Ok, Result};
use krjw_engine::{EventDriver, Timer, engine_handler::{AppMsgReceiver, run_app}, graphic};
use winit::window::{Window, WindowAttributes};

struct App {
    window: Window,
    event: EventDriver,
    gfx: graphic::D3D11,
    timer: Timer,
    ctx: Ctx,
}

struct Ctx {

}

impl Default for Ctx {
    fn default() -> Self {
        Self {
            
        }
    }
}

impl App {
    pub fn new(window: Window, hwnd: isize, rx: AppMsgReceiver) -> Result<Self> {
        let gfx = graphic::D3D11::init_on_hwnd(hwnd)?;
        let event = EventDriver::new(rx, &window);

        Ok(Self {
            window,
            event,
            gfx,
            timer: Timer::default(),
            ctx: Ctx::default(),
        })
    }
}

impl App {
    pub fn step_event(&mut self, _dt_f64: f64, _dt_f32: f32) -> Result<()> {
        Ok(())
    }

    pub fn render(&mut self, _dt_f64: f64, _dt_f32: f32) -> Result<()> {
        self.gfx.clear_screen(&[0., 0., 0., 1.]);

        self.gfx.present().context("[D3D11 GFX] Presenting failed")?;

        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            if self.event.poll_frame().to_quit() {
                break;
            }

            self.event.if_window_size_dirty(|width, height| {
                self.gfx.on_resize(width, height).context("Resizing the D3D11 graphic engine failed.")
            })?;

            let dt_f64 = self.timer.pre_frame_and_get_delta_time();
            let dt_f32 = dt_f64 as f32;

            self.step_event(dt_f64, dt_f32).context("Step processing failed")?;
            self.render(dt_f64, dt_f32).context("Rendering failed")?;

            self.timer.post_frame_fpsc(dt_f64);
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    run_app(WindowAttributes::default(), |window, hwnd, rx| {
        // 游戏线程启动
        let mut app = App::new(window, hwnd, rx).context("Creating the app instance failed")?;
        app.run().context("Error occurred when the app is running")
    }).context("App failed")
}