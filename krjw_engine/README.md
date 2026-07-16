# krjw_engine — Direct3D 11 2D Sprite Engine

基于 Direct3D 11、`winit`、`glam` 和 `kira` 的可复用 2D 精灵引擎。  
> **作者的话**：因为作者懒得造轮子，所以大部分代码是 Vibe 出来的。  

## Architecture / 架构概览

```
Main Thread (winit)          App Thread
┌────────────────┐          ┌──────────────────────┐
│  EngineHandler  │──MPSC──→│  EventDriver          │
│  (Application-  │  msg    │  ├─ KeyboardInput     │
│   Handler)      │         │  ├─ MouseInput        │
│                 │         │  └─ poll_frame()      │
└────────────────┘          └──────────┬───────────┘
                                       │
                              ┌────────▼───────────┐
                              │  App (your code)    │
                              │  ├─ update_tiles()  │
                              │  ├─ render_frame()  │
                              │  └─ …              │
                              └────────┬───────────┘
                                       │
                              ┌────────▼───────────┐
                              │  D3D11 / Sprite-    │
                              │  Batch2D / Shape-   │
                              │  Batch2D            │
                              └────────────────────┘
```

> **作者**：难以置信，这样真的可以跑

## Quick Start / 快速开始

### 1. Workspace Cargo.toml

```toml
[workspace]
members = ["krjw_engine", "my_app"]
resolver = "3"
```

### 2. 在 `my_app/Cargo.toml` 中添加依赖

```toml
[dependencies]
krjw_engine = { path = "../krjw_engine" }
anyhow = "1.0"
winit = "0.30.13"
glam = "0.29"
kira = "0.12.1"
cosmic-text = "0.19.0"
image = "0.25.10"

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.62.2", features = [
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Direct3D11",
    "Win32_Graphics_Direct3D",
    "Win32_Graphics_Dxgi",
    "Win32_Graphics_Dxgi_Common",
    "Win32_Graphics_Direct3D_Fxc",
] }
```

> **作者**：因为是半成品，所以要包括一堆 WinAPI 玩意

### 3. 最小入口 `my_app/src/main.rs`

```rust
mod app;

use krjw_engine::EngineHandler;
use winit::event_loop::ControlFlow;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new(|window, hwnd, rx| {
        let mut app = app::App::default();
        app.run(window, hwnd, rx)
    });
    event_loop.run_app(&mut handler).unwrap();
}
```

### 4. 应用骨架 `my_app/src/app.rs`

```rust
use std::sync::mpsc::Receiver;
use anyhow::Result;
use krjw_engine::*;

pub struct App {
    pub ctx: Option<AppContext>,
}

pub struct AppContext {
    // 引擎核心
    pub window: winit::window::Window,
    pub gfx: D3D11,
    pub batch: SpriteBatch2D,
    pub shape_batch: ShapeBatch2D,
    pub textures: std::collections::HashMap<String, std::sync::Arc<TextureInfo>>,
    pub camera: Camera2D,
    pub timer: Timer,
    pub text_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    pub sprite_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    // 游戏状态……
}

impl Default for App {
    fn default() -> Self {
        App { ctx: None }
    }
}

impl App {
    pub fn run(&mut self, window: winit::window::Window, hwnd: isize,
               rx: Receiver<AppMsg>) -> Result<()> {
        use windows::Win32::Foundation::HWND;

        // 1. D3D11 初始化
        let gfx = D3D11::init_on_hwnd(HWND(hwnd as *mut _))?;
        let size = window.inner_size();

        // 2. 事件驱动
        let mut driver = EventDriver::new(rx);
        driver.set_initial_window_size(size.width, size.height);

        // 3. 创建批渲染器
        let batch = SpriteBatch2D::new(&gfx.device, 2048,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_tex_rgba_2d,
            &gfx.states.input_layout_puc)?;
        let shape_batch = ShapeBatch2D::new(&gfx.device, 4096,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_solid_2d,
            &gfx.states.input_layout_puc)?;

        // 4. 主循环
        loop {
            let events = driver.poll_frame();
            if events.close_requested || events.disconnected { break; }

            // 窗口大小变化
            if driver.window_size_dirty() {
                let (w, h) = driver.window_size();
                gfx.on_resize(w, h)?;
                driver.clear_window_size_dirty();
            }

            // 你的帧逻辑
            // self.update()?;
            // self.render(&gfx, &camera)?;

            gfx.present()?;
            driver.end_frame();
        }
        Ok(())
    }
}
```
> **作者**：关于`AppContext`和`App`，作者已经在想优化方式了😂  

---

## Core Concepts / 核心概念

### Threading Model / 线程模型

引擎使用**双线程架构**：

| 线程 | 职责 | 组件 |
|------|------|------|
| Main Thread | winit 事件循环、窗口管理 | `EngineHandler` |
| App Thread | 输入处理、物理、渲染 | `EventDriver` + 你的 `App` |

- `EngineHandler` 在 `resumed()` 时创建窗口、建立 MPSC 通道、派生 App 线程
- 窗口事件（键盘/鼠标/窗口大小）通过通道转发到 App 线程
- App 线程通过 `EventDriver::poll_frame()` 一次性取出所有待处理事件

### Frame Loop / 帧循环

```
┌──────────┐    ┌───────────┐    ┌─────────┐    ┌──────────┐
│ poll_    │───→│ update    │───→│ render  │───→│ present  │
│ frame()  │    │ (物理/    │    │ (精灵/   │    │ (交换链  │
│          │    │  输入)    │    │  形状)   │    │  提交)   │
└──────────┘    └───────────┘    └─────────┘    └──────────┘
                                                    │
                                              ┌─────▼─────┐
                                              │ end_frame │
                                              │ (清除边缘  │
                                              │  状态)     │
                                              └───────────┘
```

`Timer::pre_frame_and_get_delta_time()` 应在 update 开始时调用，`Timer::post_frame_fpsc()` 在 frame 结束后调用。

---

## Module Reference / 模块参考

### `EngineHandler`

主线程 winit 事件处理器。

```rust
pub struct EngineHandler;

impl EngineHandler {
    /// 创建处理器。`app_init` 是一个闭包，接收 (Window, HWND, Receiver<AppMsg>)，
    /// 在专用线程上运行应用。
    pub fn new(
        app_init: impl FnOnce(Window, isize, Receiver<AppMsg>) -> Result<()>
            + Send + 'static
    ) -> Self;
}
```

### `msg::AppMsg`

主线程 → App 线程的消息枚举。

```rust
pub enum AppMsg {
    CloseRequested,
    Resized(u32, u32),
    Moved(i32, i32),
    KeyboardInput { key_code: KeyCode, state: ElementState },
    CursorMoved(f64, f64),
    CursorEntered,
    CursorLeft,
    MouseWheel(f64, f64),
    MouseWheelPixel(f64, f64),
    MouseInput { button: MouseButton, state: ElementState },
    MouseMotion(f64, f64),
}
```

所有变体均为 `Send + Copy`。

### `EventDriver`

App 线程的消息驱动。提供：

```rust
pub fn poll_frame(&mut self) -> FrameEvents;  // 取出所有待处理事件
pub fn keyboard(&self) -> &KeyboardInput;       // 键盘状态
pub fn mouse(&self) -> &MouseInput;             // 鼠标状态
pub fn window_size(&self) -> (u32, u32);
pub fn window_size_dirty(&self) -> bool;
pub fn clear_window_size_dirty(&mut self);
pub fn end_frame(&mut self);                    // 推进边缘状态
```

### Input System / 输入系统

**按键状态** (`KeyState`) 使用位掩码：

```rust
let ks = driver.keyboard().get_key_state(KeyCode::KeyW);
ks.is_pressed();           // 是否按下
ks.is_down_edge();         // 本帧刚按下（边缘触发）
ks.is_down_true_edge();    // 本帧刚按下（经过去抖）
```

**鼠标状态**：

```rust
driver.mouse().get_mouse_pos_vec2();             // 光标位置 Vec2
driver.mouse().get_mouse_button_state(Left);     // 按键状态
driver.mouse().get_mouse_wheel_delta();          // 滚轮行增量
driver.mouse().get_pixel_wheel();                // 滚轮像素增量
```

**辅助宏**（建议在你的 app.rs 中定义）：

```rust
macro_rules! key_pressed { ($driver:expr, $key:expr) => {
    $driver.keyboard().get_key_state($key).is_pressed()
}}
macro_rules! key_state { ($driver:expr, $key:expr) => {
    $driver.keyboard().get_key_state($key)
}}
```

### `D3D11`

Direct3D 11 设备封装。

```rust
pub fn init_on_hwnd(hwnd: HWND) -> Result<Self>;
pub fn init_on_window(window: &Window) -> Result<Self>;
pub fn present(&self) -> Result<()>;
pub fn clear_screen(&self, color_rgba: &[f32; 4]);
pub fn set_viewport(&self, top: f32, left: f32, width: f32, height: f32);
pub fn on_resize(&mut self, width: u32, height: u32) -> Result<()>;

// 公开字段
pub device: ID3D11Device;
pub swap_chain: IDXGISwapChain;
pub imm_context: ID3D11DeviceContext;
pub states: StateObjects;
```

`StateObjects` 包含：

| 类别 | 资源 |
|------|------|
| Blend | `blend_opaque`, `blend_alpha`, `blend_additive` |
| Sampler | `sampler_point_clamp`, `sampler_linear_clamp`, `sampler_linear_wrap` |
| Rasterizer | `rasterizer_solid_cull_none`, `rasterizer_solid_cull_back`, `rasterizer_wireframe` |
| Depth | `depth_none`, `depth_less` |
| Shaders | `vs_puc_m_2d`, `ps_solid_2d`, `ps_tex_rgba_2d`, `ps_tex_r8_2d` |
| InputLayout | `input_layout_puc` |

### `SpriteBatch2D`

基于 `D3D11` 的精灵批渲染器（4 顶点/quad，索引 16-bit）。

| 方法 | 说明 |
|------|------|
| `new(device, capacity, vs, ps, input_layout)` | 创建批渲染器 |
| `set_texture(srv, width, height)` | 绑定当前纹理 |
| `add(pos, scale, rot, sprite, color)` | 添加一个精灵（自动绑定变换） |
| `set_mvp(gfx, &matrix)` | 设置 MVP 矩阵 |
| `submit_and_draw(gfx)` | 提交并绘制 |
| `clear_batch()` | 清空当前帧精灵 |
| `push_buffered(gfx, vp, buf, extract_fn)` | **高级**：将 `Sprite2DBuffer` 按 pipeline 排序后提交 |

`Pipeline` trait（绑定纹理到 batch）：

```rust
pub trait Pipeline: HaveID + Clone {
    fn apply_to_batch(&self, batch: &mut SpriteBatch2D);
}
```

> **作者**：这是不是有点太怪了？好吧准确来说这个`trait`应该只会在`sprite_batch_2d.rs`里用，如果可以往`shape_batch_2d.rs`……  

`TextureInfoArced` 提供了 `Pipeline` 的实现，直接用 `Arc<TextureInfo>` 绑定纹理。

### `ShapeBatch2D`

形状批渲染器（线框、圆、无纹理）。

```rust
pub fn new(device, capacity, vs, ps, input_layout) -> Result<Self>;
pub fn add_square_line_no_uv(p1, p2, thickness, color);  // 线段
pub fn add_circle_no_uv(center, radius, color, segments); // 圆形轮廓
pub fn set_mvp(gfx, &matrix);
pub fn submit_and_draw(gfx) -> Result<()>;
pub fn clear_batch();
```

### `Sprite2D` / `Sprite2DBuffer` / `Sprite2DObject`

**`Sprite2D`** — 精灵 UV 描述符（所有值以像素为单位）：

```rust
pub struct Sprite2D {
    pub origin_px: Vec2,     // 原点/轴点
    pub size_px: Vec2,       // 渲染尺寸
    pub uv_tl_px: Vec2,      // 纹理左上角 UV（像素）
    pub uv_size_px: Vec2,    // UV 矩形尺寸
}
```

**`Sprite2DBuffer<T, U>`** — 按 pipeline 排序的精灵缓存：

```rust
pub fn push(&mut self, obj: &Sprite2DObject<T, U>);
pub fn clear(&mut self);
pub fn for_each_sorted(&mut self, ex, on_pipeline_change, on_item);
```

**`Sprite2DObject<T, U>`** — 完整的精灵对象：

```rust
pub struct Sprite2DObject<T: HaveID + Clone, U: Clone> {
    pub spr: Sprite2D,
    pub color: [f32; 4],
    pub transform: U,          // 如 Transform2D
    pub pipeline: T,           // 如 TextureInfoArced
    pub layer: f64,            // 排序层级
}
```

### `Transform2D`

RST 变换（旋转 → 缩放 → 平移）。

```rust
pub struct Transform2D {
    pub pos: Vec2,
    pub scale: Vec2,
    pub rot: f32,
}
// 方法: with_pos, with_scale, with_rot, move_by, scale_by, rotate_by
//       transform(parent), transform_point(local), inverse_transform_point(world)
```

### `Camera2D`

正交相机。

```rust
pub struct Camera2D {
    pub position: Vec2,
    pub rotation: f32,
    pub zoom: Vec2,
    pub viewport_pos: Vec2,
    pub viewport_size: Vec2,
}

pub fn vp_matrix(&self) -> Mat4;          // View-Projection 矩阵
pub fn apply_viewport(&self, gfx: &D3D11);// 设置 GPU 视口
pub fn screen_to_world(&self, px) -> Vec2;
pub fn world_to_screen(&self, world) -> Vec2;
```
> **作者**：这`apply_viewport`是不是和 Direct3D11 有点太耦合了？

### `Collider` / `ColliderInstance`

碰撞形状与碰撞检测。

```rust
pub enum Collider {
    AABB { half_size: Vec2 },      // 轴对齐包围盒（忽略旋转）
    Rect { half_size: Vec2 },      // 有向包围盒（应用完整变换）
    Circle { radius: f32 },
}

pub struct ColliderInstance<'a> {
    pub shape: &'a Collider,
    pub xform: Transform2D,
}

// 方法:
pub fn contains_point(&self, point: Vec2) -> bool;
pub fn overlaps(&self, other: &ColliderInstance) -> Overlap;
```

### `AtlasText`

动态文字图集（cosmic-text 排版 + swash 光栅化 + skyline 打包）。

```rust
pub fn new(device, lifetime_a: f32, lifetime_b: f32) -> Result<Self>;
pub fn layout_text(&mut self, text, metrics, attrs, device) -> Result<TextLayout>;
/// 返回的 TextLayout 包含 `content_size: Vec2`，可用于精确定位。
pub fn render_layout(&self, layout, offset, origin, transform, color, layer, buffer);
pub fn render_layout_simple(&self, layout, offset, color, layer, buffer);
pub fn upload(&mut self, gfx: &D3D11) -> Result<()>;
```
> **作者**：那为什么没有单独的`DynamicAtlas`呢？

### `Timer`

帧计时器（FPS + delta time）。

```rust
pub fn pre_frame_and_get_delta_time(&mut self) -> f64;
pub fn post_frame_fpsc(&mut self, dt: f64);
pub fn get_fps(&self) -> f64;
```

---

## Complete Example / 完整示例

参见 `app_sethsweeper` crate，它展示了完整的用法：

- 音频初始化 (`kira::AudioManager`)
- 纹理加载 (`d3d11_utils::load_texture_from_dynamic_image`)
- 精灵批渲染（带 shadow + push_buffered）
- 网格绘制 (`ShapeBatch2D`)
- 碰撞体轮廓绘制 (`ColliderInstance`)
- 动态文字 HUD (`AtlasText`)
- 相机控制（WASD 移动、滚轮缩放）

> **作者**：虽然乱就是了

---

## Platform / 平台

当前仅支持 **Windows (x64)**，基于 Direct3D 11。

> **作者**：已经在实现 D3D11 Backend 的时候尽力考虑跨平台的事了😂😂😂  