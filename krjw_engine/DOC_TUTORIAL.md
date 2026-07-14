# KRJW_Engine 完整文档与教程

> 基于 Direct3D 11、`winit`、`glam` 和 `kira` 的可复用 2D 精灵引擎。  
> **作者的话**：因为作者懒得造轮子，所以大部分代码是 Vibe 出来的。  

---

## 目录

1. [概述与架构](#1-概述与架构)
2. [快速开始](#2-快速开始)
3. [模块详解](#3-模块详解)
   - 3.1 [EngineHandler](#31-enginehandler--入口)
   - 3.2 [AppMsg](#32-appmsg--消息枚举)
   - 3.3 [EventDriver & FrameEvents](#33-eventdriver--frameevents)
   - 3.4 [KeyState](#34-keystate--按键状态位掩码)
   - 3.5 [KeyboardInput](#35-keyboardinput)
   - 3.6 [MouseInput & MouseButton](#36-mouseinput--mousebutton)
   - 3.7 [Timer](#37-timer)
   - 3.8 [D3D11](#38-d3d11)
   - 3.9 [StateObjects](#39-stateobjects)
   - 3.10 [TextureInfo & d3d11_utils](#310-textureinfo--d3d11_utils)
   - 3.11 [Sprite2D / Sprite2DObject / Sprite2DBuffer / HaveID](#311-sprite2d--sprite2dobject--sprite2dbuffer--haveid)
   - 3.12 [TextureInfoArced](#312-textureinfoorced)
   - 3.13 [SpriteBatch2D & Pipeline](#313-spritebatch2d--pipeline-trait)
   - 3.14 [ShapeBatch2D](#314-shapebatch2d)
   - 3.15 [Transform2D](#315-transform2d)
   - 3.16 [Camera2D](#316-camera2d)
   - 3.17 [Collider / ColliderInstance / Overlap](#317-collider--colliderinstance--overlap)
   - 3.18 [AtlasText & TextLayout](#318-atlastext--textlayout)
4. [完整示例](#4-完整示例)
5. [附录：已知问题与未来规划](#5-附录已知问题与未来规划)

---

## 1. 概述与架构

### 1.1 线程模型

引擎使用**双线程架构**：

| 线程 | 职责 | 组件 |
|------|------|------|
| **Main Thread** | winit 事件循环、窗口管理 | `EngineHandler` |
| **App Thread** | 输入处理、物理、渲染 | `EventDriver` + 你的 `App` |

- `EngineHandler` 在 `resumed()` 时创建窗口、建立 MPSC 通道、派生 App 线程
- 窗口事件（键盘、鼠标、窗口大小）通过通道转发到 App 线程
- App 线程通过 `EventDriver::poll_frame()` 一次性取出所有待处理事件

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

### 1.2 帧循环

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

### 1.3 平台支持

当前仅支持 **Windows (x64)**，基于 Direct3D 11。

⚠️ **Warning**：引擎使用了大量 Windows/Win32 API，跨平台移植需要重写整个 `graphic::d3d11` 模块。

---

## 2. 快速开始

### 2.1 Workspace Cargo.toml

```toml
[workspace]
members = ["krjw_engine", "my_app"]
resolver = "3"
```

### 2.2 在 `my_app/Cargo.toml` 中添加依赖

```toml
[package]
name = "my_app"
version = "0.1.0"
edition = "2024"

[dependencies]
krjw_engine = { path = "../krjw_engine" }
anyhow = "1.0"
glam = "0.29"
```

### 2.3 最小入口 `my_app/src/main.rs`

```rust
mod app;

use krjw_engine::EngineHandler;
use krjw_engine::winit::event_loop::{ControlFlow, EventLoop};

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new(|window, hwnd, rx| {
        let mut app = app::App::default();
        app.run(window, hwnd, rx)
    });
    event_loop.run_app(&mut handler).unwrap();
}
```

### 2.4 应用骨架 `my_app/src/app.rs`

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
        // 1. D3D11 初始化
        let gfx = D3D11::init_on_hwnd(hwnd)?;
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

---

## 3. 模块详解

---

### 3.1 EngineHandler — 入口

**文件**：`engine_handler.rs`  
**公开导出**：`krjw_engine::EngineHandler`

主线程的 winit 事件处理器。负责创建窗口、建立 MPSC 通道、派生应用线程。

#### 构造

```rust
pub fn new(
    app_init: impl FnOnce(Window, isize, Receiver<AppMsg>) -> Result<()>
        + Send + 'static
) -> Self;
```

- `app_init`：在派生线程中调用的闭包，接收 `(Window, HWND isize, Receiver<AppMsg>)`
- 闭包应运行你的 App 主循环，返回 `Result<()>`

#### 内部行为

- `resumed()` 时创建窗口（标题硬编码为 `"KrisuRJW"`，960×600），解析 HWND，创建通道，派生线程
- `window_event()` 将所有 WindowEvent 转为 `AppMsg` 并发送到通道
- `device_event()` 将 `MouseMotion` 事件转发
- `Drop` 时发送通道关闭信号

#### Example

```rust
fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut handler = EngineHandler::new(|window, hwnd, rx| {
        // 这里运行你的 App
        my_app::run(window, hwnd, rx)
    });
    event_loop.run_app(&mut handler).unwrap();
}
```

⚠️ **Warning**：
- 窗口标题、尺寸硬编码在 `resumed()` 中，没有提供自定义接口
- 关闭窗口时调用 `event_loop.exit()`，可能导致 App 线程仍在运行就被终止
- 仅支持 Win32 平台，非 Windows 系统会 panic

🚧 **TODO**：
- 允许用户自定义窗口属性（标题、尺寸、模式等）
- 提供更优雅的退出机制（等待 App 线程自然结束）
- 跨平台窗口句柄支持

---

### 3.2 AppMsg — 消息枚举

**文件**：`msg.rs`  
**公开导出**：`krjw_engine::AppMsg`

主线程通过 MPSC 通道发送给 App 线程的所有事件类型。

```rust
#[derive(Debug, Clone, Copy)]
pub enum AppMsg {
    CloseRequested,                      // 窗口关闭
    Resized(u32, u32),                   // 窗口大小变化 (width, height)
    Moved(i32, i32),                     // 窗口移动 (x, y)
    KeyboardInput { key_code: KeyCode, state: ElementState },
    CursorMoved(f64, f64),               // 光标移动 (x, y)
    CursorEntered,                       // 光标进入窗口
    CursorLeft,                          // 光标离开窗口
    MouseWheel(f64, f64),                // 滚轮行增量 (x, y)
    MouseWheelPixel(f64, f64),           // 滚轮像素增量 (x, y)
    MouseInput { button: MouseButton, state: ElementState },
    MouseMotion(f64, f64),               // 鼠标原生移动增量 (dx, dy)
}
```

所有变体均使用 `Send + Copy` 类型，因此 `AppMsg` 自身也是 `Send`。

---

### 3.3 EventDriver & FrameEvents

**文件**：`event_driver.rs`  
**公开导出**：`krjw_engine::EventDriver`, `krjw_engine::FrameEvents`

App 线程的消息驱动，从通道接收消息并更新输入/窗口状态。

#### FrameEvents

```rust
pub struct FrameEvents {
    pub close_requested: bool,   // 是否收到关闭请求
    pub disconnected: bool,      // 通道是否断开（发送端丢弃）
}
```

#### 方法

```rust
impl EventDriver {
    pub fn new(rx: Receiver<AppMsg>) -> Self;
    pub fn set_initial_window_size(&mut self, w: u32, h: u32);

    // 帧管理
    pub fn poll_frame(&mut self) -> FrameEvents;   // 取出所有待处理消息
    pub fn end_frame(&mut self);                   // 推进边缘状态

    // 输入访问
    pub fn keyboard(&self) -> &KeyboardInput;
    pub fn mouse(&self) -> &MouseInput;

    // 窗口状态
    pub fn window_pos(&self) -> (i32, i32);
    pub fn window_size(&self) -> (u32, u32);
    pub fn window_size_dirty(&self) -> bool;
    pub fn clear_window_size_dirty(&mut self);
}
```

#### Example

```rust
loop {
    let events = driver.poll_frame();
    if events.close_requested || events.disconnected { break; }

    if driver.window_size_dirty() {
        let (w, h) = driver.window_size();
        gfx.on_resize(w, h)?;
        driver.clear_window_size_dirty();
    }

    // 使用输入
    let ks = driver.keyboard().get_key_state(KeyCode::KeyW);
    let mouse_pos = driver.mouse().get_mouse_pos_vec2();

    driver.end_frame();
}
```

⚠️ **Warning**：`poll_frame()` 使用 `try_recv()` 非阻塞读取，不会等待消息。如果你的 App 逻辑与帧率不同步（例如需要等待输入才更新），需要自行实现同步机制。

🚧 **TODO**：可以考虑提供阻塞版本的 `poll_frame` 以支持等待事件驱动的模式。

---

### 3.4 KeyState — 按键状态位掩码

**文件**：`key_state.rs`  
**公开导出**：`krjw_engine::KeyState`, `krjw_engine::KEY_STATE_*` 常量

使用位掩码表示按键的当前状态，支持按下/释放检测和边缘触发。

#### 位掩码常量

| 常量 | 值 | 含义 |
|------|-----|------|
| `KEY_STATE_RELEASED` | 0b0000 | 已释放 |
| `KEY_STATE_PRESSING` | 0b0001 | 按下中 |
| `KEY_STATE_UP_EDGE` | 0b0010 | 刚释放（普通边缘） |
| `KEY_STATE_DOWN_EDGE` | 0b0011 | 刚按下（普通边缘） |
| `KEY_STATE_UP_TRUE_EDGE` | 0b0110 | 刚释放（真边缘） |
| `KEY_STATE_DOWN_TRUE_EDGE` | 0b0111 | 刚按下（真边缘） |

#### 方法

```rust
impl KeyState {
    pub fn is_pressed(&self) -> bool;
    pub fn is_released(&self) -> bool;
    pub fn is_edge(&self) -> bool;          // 是否有边缘触发
    pub fn is_true_edge(&self) -> bool;     // 是否是真边缘
    pub fn is_up_edge(&self) -> bool;       // 是否上升沿
    pub fn is_down_edge(&self) -> bool;     // 是否下降沿
    pub fn is_up_true_edge(&self) -> bool;
    pub fn is_down_true_edge(&self) -> bool;
    pub fn off_edge(&self) -> KeyState;     // 清除边缘位（每帧结束后调用）
}
```

#### 边缘检测（Edge Detection）说明

- **普通边缘**（`DOWN_EDGE` / `UP_EDGE`）：OS 报告的状态变化。某些 OS 可能连续发送相同状态的重复事件。
- **真边缘**（`DOWN_TRUE_EDGE` / `UP_TRUE_EDGE`）：仅当状态确实从相反状态切换时才为真。用于需要"恰好按一次"的场景（如跳跃、射击）。

| 场景 | 普通边缘 | 真边缘 |
|------|---------|-------|
| 按住按键 → 连续收到 Pressed 事件 | 每帧触发 | 仅第一次触发 |
| 实际按下一次 | 触发一次 | 触发一次 |
| 去抖需求 | ❌ | ✅ |

#### Example

```rust
let ks = driver.keyboard().get_key_state(KeyCode::KeyW);

// 持续按住（模拟移动）
if ks.is_pressed() {
    player.move_forward(dt);
}

// 单次触发（跳跃、射击）
if ks.is_down_true_edge() {
    player.jump();
}
```

⚠️ **Warning**：`is_down_edge()` 和 `is_down_true_edge()` 的返回在每帧结束后通过 `off_edge()` 清除。如果在同一帧内多次查询同一个 KeyState，返回结果一致。确保在 `end_frame()` 之前完成所有输入处理。

---

### 3.5 KeyboardInput

**文件**：`keyboard_input.rs`  
**公开导出**：`krjw_engine::KeyboardInput`

管理所有键盘按键的状态，使用 `HashMap<KeyCode, KeyState>`。

#### 方法

```rust
impl KeyboardInput {
    pub fn get_key_state(&self, key_code: KeyCode) -> KeyState;
    pub fn get_keys_iter(&self) -> impl Iterator<Item = (KeyCode, KeyState)> + '_;
    pub fn end_frame(&mut self);        // 清除所有边缘位
}
```

#### 建议辅助宏（在你的 `app.rs` 中定义）

```rust
macro_rules! key_pressed { ($driver:expr, $key:expr) => {
    $driver.keyboard().get_key_state($key).is_pressed()
}}
macro_rules! key_state { ($driver:expr, $key:expr) => {
    $driver.keyboard().get_key_state($key)
}}
```

#### 使用 Example

```rust
// 同时检测多个按键
if key_pressed!(driver, KeyCode::KeyW) {
    pos.y -= speed * dt;
}
if key_pressed!(driver, KeyCode::KeyS) {
    pos.y += speed * dt;
}
if key_pressed!(driver, KeyCode::KeyA) {
    pos.x -= speed * dt;
}
if key_pressed!(driver, KeyCode::KeyD) {
    pos.x += speed * dt;
}

// 真边缘检测（单次事件）
if key_state!(driver, KeyCode::Space).is_down_true_edge() {
    fire_bullet();
}
```

---

### 3.6 MouseInput & MouseButton

**文件**：`mouse_input.rs`  
**公开导出**：`krjw_engine::MouseInput`, `krjw_engine::MouseButton`

管理鼠标位置、按钮状态、滚轮增量。

#### MouseButton

```rust
pub enum MouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
    X1 = 3,    // 后退键
    X2 = 4,    // 前进键
}
```

#### MouseInput 方法

```rust
impl MouseInput {
    pub fn get_mouse_position(&self) -> (f64, f64);
    pub fn get_mouse_pos_vec2(&self) -> Vec2;          // 便捷返回 glam::Vec2
    pub fn get_mouse_delta(&self) -> (f64, f64);        // 原生鼠标移动增量
    pub fn get_mouse_button_state(&self, button: MouseButton) -> KeyState;
    pub fn get_mouse_wheel_delta(&self) -> (f64, f64);  // 滚轮行在一帧内的增量
    pub fn get_pixel_wheel(&self) -> Option<(f64, f64)>; // 滚轮像素在一帧内增量（触控板等），如果不可用则返回 None，否则返回 Some((f64, f64))，分别代表 x、y 方向的像素增量
    pub fn is_in_window(&self) -> bool;
    pub fn get_mouse_button_states_iter(&self) -> impl Iterator<Item = (MouseButton, KeyState)> + '_;
    pub fn end_frame(&mut self);   // 清除边缘位和滚轮增量
}
```

#### Example

```rust
// 获取鼠标位置
let mouse_pos = driver.mouse().get_mouse_pos_vec2();

// 鼠标左键点击检测
if driver.mouse().get_mouse_button_state(MouseButton::Left).is_down_true_edge() {
    println!("Left click at {:?}", mouse_pos);
}

// 滚轮缩放
let (_, wheel_y) = driver.mouse().get_mouse_wheel_delta();
camera.zoom *= 1.0 - (wheel_y as f32) * 0.1;

// 鼠标中键拖拽
if driver.mouse().get_mouse_button_state(MouseButton::Middle).is_pressed() {
    let (dx, dy) = driver.mouse().get_mouse_delta();
    camera.position.x -= dx as f32;
    camera.position.y -= dy as f32;
}
```

⚠️ **Warning**：
- `mouse_delta` 来自 `DeviceEvent::MouseMotion`，是**原始增量**，不受操作系统鼠标加速/指针速度影响。适用于 FPS 相机控制。
- `mouse_position` 来自 `WindowEvent::CursorMoved`，是窗口坐标。
- `pixel_wheel` 和 `mouse_wheel_delta` 每帧结束时被清零，请在 `end_frame()` 前使用。

🚧 **TODO**：
- `MouseInput::device_event` 中的 `MouseWheel` 处理被注释掉了（行 212-225）。如果需要通过设备事件接收滚轮，需取消注释。

---

### 3.7 Timer

**文件**：`timer.rs`  
**公开导出**：`krjw_engine::Timer`

帧计时器，提供 Delta Time 和 FPS 统计。

#### 方法

```rust
impl Timer {
    pub fn pre_frame_and_get_delta_time(&mut self) -> f64; // 返回距上一帧的秒数
    pub fn post_frame_fpsc(&mut self, dt: f64);            // 更新 FPS（EMA 平滑）
    pub fn get_fps(&self) -> f64;                          // 获取当前平滑 FPS
}
```

#### Example

```rust
// 帧开始
let dt = timer.pre_frame_and_get_delta_time();
// 如果你希望 dt 有上限（防止大跳帧时间步长过大）：
let dt = dt.min(0.05);

// 更新逻辑
player.update(dt);

// 帧结束
timer.post_frame_fpsc(dt);

// 显示 FPS
println!("FPS: {:.1}", timer.get_fps());
```

#### FPS 计算公式

使用指数移动平均（EMA），α = 0.1：
```
instant_fps = 1.0 / dt
fps = fps * 0.9 + instant_fps * 0.1
```

🚧 **TODO**：可以考虑将 EMA 常数 α 作为可配置参数。

---

### 3.8 D3D11

**文件**：`graphic/d3d11.rs`  
**公开导出**：`krjw_engine::D3D11`

Direct3D 11 设备封装，管理设备、交换链、渲染目标、深度模板。

#### 公开字段

```rust
pub struct D3D11 {
    pub device: ID3D11Device,          // D3D 设备
    pub swap_chain: IDXGISwapChain,    // 交换链
    pub imm_context: ID3D11DeviceContext, // 即时上下文
    pub states: StateObjects,          // 预创建的渲染状态对象
    // (以下为私有字段)
    // render_target_view
    // depth_stencil_texture
    // depth_stencil_view
}
```

#### 方法

```rust
impl D3D11 {
    // 初始化
    pub fn init_on_hwnd(hwnd: isize) -> Result<Self>; // isize 对应 winit 库对 HWND 的定义
    pub fn init_on_window(window: &Window) -> Result<Self>;

    // 渲染控制
    pub fn clear_screen(&self, color_rgba: &[f32; 4]);
    pub fn set_viewport(&self, top_x: f32, top_y: f32, width: f32, height: f32);
    pub fn present(&self) -> Result<()>;

    // 窗口大小变化
    pub fn on_resize(&mut self, width: u32, height: u32) -> Result<()>;
    pub fn reset_rtv(&mut self, width: u32, height: u32, format: DXGI_FORMAT, flags: DXGI_SWAP_CHAIN_FLAG) -> Result<()>;
    pub fn reset_dsv(&mut self, width: u32, height: u32) -> Result<()>;

    // 视图访问
    pub fn rtv(&self) -> &ID3D11RenderTargetView;
    pub fn dsv(&self) -> &ID3D11DepthStencilView;
}
```

#### Example

```rust
// 初始化
let gfx = D3D11::init_on_hwnd(HWND(hwnd as *mut _))?;

// 帧渲染
gfx.clear_screen(&[0.1, 0.1, 0.2, 1.0]);

// ... 绘制内容 ...

gfx.present()?;

// 窗口大小变化
gfx.on_resize(new_width, new_height)?;
```

⚠️ **Warning**：
- `init_on_hwnd` 仅支持 Win32 窗口，没有跨平台能力
- `present()` 固定使用 `1` 作为 `SyncInterval`（等待垂直同步），不可配置
- `init_on_hwnd` 中硬编码创建了 2 个后备缓冲（`BufferCount: 2`）+ `DXGI_SWAP_EFFECT_FLIP_DISCARD`
- Debug 模式下自动启用 `D3D11_CREATE_DEVICE_DEBUG`，如果没有安装 Direct3D Debug Layer 会报错

🚧 **TODO**：
- 可配置的 vsync 开关（`SyncInterval` 可设置为 0 或 1）
- 可配置的后备缓冲数量
- 支持无深度模板的渲染路径（纯 2D 场景可能不需要深度测试）

---

### 3.9 StateObjects

**文件**：`graphic/d3d11/state_objects.rs`  
在 `D3D11::states` 中访问，自动创建。

#### 所有预创建资源

| 类别 | 名称 | 说明 |
|------|------|------|
| **混合状态** | `blend_opaque` | 不透明（Blend 关闭） |
| | `blend_alpha` | Alpha 透明（`SRC_ALPHA, INV_SRC_ALPHA`） |
| | `blend_additive` | 叠加混合（`SRC_ALPHA, ONE`） |
| **采样器** | `sampler_point_clamp` | 点采样 + Clamp |
| | `sampler_linear_clamp` | 线性采样 + Clamp |
| | `sampler_linear_wrap` | 线性采样 + Wrap |
| **光栅化** | `rasterizer_solid_cull_none` | 实体无剔除 |
| | `rasterizer_solid_cull_back` | 实体背面剔除 |
| | `rasterizer_wireframe` | 线框模式 |
| **深度模板** | `depth_none` | 深度测试关闭 |
| | `depth_less` | 深度测试开启（Less） |
| **纹理** | `white_texture_srv` | 1×1 白色像素纹理（用于纯色渲染） |
| **着色器** | `vs_puc_m_2d` | 2D 顶点着色器（pos + uv + color + MVP） |
| | `ps_solid_2d` | 纯色像素着色器（忽略纹理） |
| | `ps_tex_rgba_2d` | RGBA 纹理像素着色器 |
| | `ps_tex_r8_2d` | R8 单通道纹理像素着色器 |
| **输入布局** | `input_layout_puc` | POSITION + TEXCOORD + COLOR 布局 |

#### Example — 设置混合状态

```rust
// 使用 alpha 混合
unsafe {
    gfx.imm_context.OMSetBlendState(
        Some(&gfx.states.blend_alpha),
        None,
        0xFFFFFFFF,
    );
}

// 使用线框模式调试
unsafe {
    gfx.imm_context.RSSetState(Some(&gfx.states.rasterizer_wireframe));
}
```

⚠️ **Warning**：引擎 `vs_puc_m_2d` 和 `ps_*` 着色器通过 `include_bytes!` 硬编码，没有提供创建自定义着色器的接口。如果需要自定义着色器，需要手动调用 `d3d11_utils` 的 `create_vs` / `create_ps` 函数。

---

### 3.10 TextureInfo & d3d11_utils

**文件**：`graphic/d3d11/d3d11_utils.rs`  
**公开导出**：`krjw_engine::TextureInfo`

工具函数模块，提供纹理、缓冲区、着色器的创建。

#### TextureInfo

```rust
pub struct TextureInfo {
    pub texture: ID3D11Texture2D,
    pub srv: ID3D11ShaderResourceView,
    pub width: u32,
    pub height: u32,
    pub format: DXGI_FORMAT,
}

impl TextureInfo {
    pub fn size_vec2f(&self) -> Vec2;   // 返回 (width, height) 作为 Vec2
}
```

#### 关键工具函数

```rust
// 着色器
pub fn compile_shader(source: &[u8], entrypoint: PCSTR, target: PCSTR) -> Result<Vec<u8>>;
pub fn create_vs(device: &ID3D11Device, hlsl_bytes: &[u8]) -> Result<ID3D11VertexShader>;
pub fn create_ps(device: &ID3D11Device, hlsl_bytes: &[u8]) -> Result<ID3D11PixelShader>;
pub fn create_input_layout(device, desc: &[D3D11_INPUT_ELEMENT_DESC], vs_blob: &[u8]) -> Result<ID3D11InputLayout>;

// 缓冲区
pub fn create_dynamic_buffer(device, byte_width, bind_flags) -> Result<ID3D11Buffer>;
pub fn create_immutable_buffer(device, data, bind_flags) -> Result<ID3D11Buffer>;
pub fn create_constant_buffer<T>(device) -> Result<ID3D11Buffer>;
pub fn write_buffer<T>(context, buffer, data) -> Result<()>;
pub fn as_u8_slice<T>(data: &[T]) -> &[u8];

// 纹理
pub fn create_texture_2d(device, width, height, format, bind_flags, usage, cpu_access, initial_data) -> Result<ID3D11Texture2D>;
pub fn create_srv(device, texture, format) -> Result<ID3D11ShaderResourceView>;
pub fn load_texture_from_dynamic_image(device, img: &DynamicImage) -> Result<TextureInfo>;
```

#### Example — 从图片文件加载纹理

```rust
use image::io::Reader as ImageReader;

let img = ImageReader::open("assets/sprite.png")?.decode()?;
let tex_info = d3d11_utils::load_texture_from_dynamic_image(&gfx.device, &img)?;

// 存储到纹理管理器
textures.insert("sprite".to_string(), Arc::new(tex_info));
```

⚠️ **Warning**：
- `create_texture_2d` 的 `cpu_access_flags` 参数类型是 `u32` 而不是 `D3D11_CPU_ACCESS_FLAG`，使用时需要注意类型转换
- `create_texture_2d` 不支持多 Mip Level 创建（`MipLevels: 1` 硬编码）
- `load_texture_from_dynamic_image` 对 RGB8 格式进行了 R8G8B8→R8G8B8A8 扩展（增加 alpha 通道为 255），这是 D3D11 的限制
- HDR 图片（Rgb32F）也会被扩展为 RGBA32F

🚧 **TODO**：
- 支持 MipMap 生成
- 支持直接从文件路径加载纹理的便捷函数
- 改进类型安全（将 `cpu_access_flags` 改为合适的枚举类型）

---

### 3.11 Sprite2D / Sprite2DObject / Sprite2DBuffer / HaveID

**文件**：`sprite2d.rs`  
**公开导出**：`krjw_engine::Sprite2D`, `krjw_engine::Sprite2DObject`, `krjw_engine::Sprite2DBuffer`, `krjw_engine::HaveID`

描述 2D 精灵的核心类型，以及按 pipeline 排序的缓冲区。

#### HaveID

```rust
pub trait HaveID {
    fn get_id(&self) -> u64;   // 返回唯一标识符
}
```

用于在排序迭代中检测 pipeline（纹理/着色器）切换。

#### Sprite2D

```rust
pub struct Sprite2D {
    pub origin_px: Vec2,     // 原点/轴点（像素），例如 center = size_px * 0.5
    pub size_px: Vec2,       // 渲染尺寸（像素）
    pub uv_tl_px: Vec2,      // 纹理左上角 UV 坐标（像素）
    pub uv_size_px: Vec2,    // UV 矩形尺寸（像素）
}
```

所有值以**像素**为单位（非归一化 UV），因为 SpriteBatch2D 内部会自动根据纹理尺寸归一化。

#### Sprite2DObject

```rust
pub struct Sprite2DObject<T: HaveID + Clone, U: Clone> {
    pub spr: Sprite2D,               // 精灵几何描述符
    pub color: [f32; 4],            // RGBA 颜色（预乘 alpha）
    pub transform: U,               // 每精灵变换（如 Transform2D）
    pub pipeline: T,                // 流水线引用（如 TextureInfoArced）
    pub layer: f64,                 // 排序层级（值越小越先绘制）
}
```

#### Sprite2DBuffer

```rust
pub struct Sprite2DBuffer<T: HaveID + Clone, U: Clone> {
    // ...
}

impl<T: HaveID + Clone, U: Clone> Sprite2DBuffer<T, U> {
    pub fn reserve(&mut self, additional: usize);
    pub fn len(&self) -> usize;
    pub fn push(&mut self, sprite: &Sprite2DObject<T, U>);  // O(1) 插入
    pub fn sort(&mut self);            // 惰性排序（自动调用）
    pub fn clear(&mut self);
    pub fn for_each_sorted<B, F, G>(
        &mut self,
        ex: &mut B,                    // 外部上下文
        on_pipeline_change: F,         // pipeline 变化时调用
        on_item: G,                    // 每个精灵调用一次
    );
}
```

排序逻辑：先按 `layer` 升序，再按 `pipeline.get_id()` 升序。

#### Example

```rust
// 准备精灵对象
let sprite_obj = Sprite2DObject {
    spr: Sprite2D {
        origin_px: Vec2::new(16.0, 16.0),  // 中心点为原点
        size_px: Vec2::new(32.0, 32.0),
        uv_tl_px: Vec2::ZERO,
        uv_size_px: Vec2::new(32.0, 32.0),
    },
    color: [1.0, 1.0, 1.0, 1.0],
    transform: Transform2D {
        pos: Vec2::new(100.0, 200.0),
        scale: Vec2::ONE,
        rot: 0.0,
    },
    pipeline: TextureInfoArced(texture.clone()),
    layer: 10.0,
};

// 推入缓冲区
sprite_buf.push(&sprite_obj);

// 排序 + 提交绘制（通过 SpriteBatch2D::push_buffered）
batch.push_buffered(&gfx, &vp, &mut sprite_buf, |t| (t.pos, t.scale, t.rot));
```

---

### 3.12 TextureInfoArced

**文件**：`lib.rs`  
**公开导出**：`krjw_engine::TextureInfoArced`

`Arc<TextureInfo>` 的包装器，同时实现 `HaveID` 和 `Pipeline` trait，用于在 `Sprite2DBuffer` 中使用。

```rust
#[derive(Debug, Clone)]
pub struct TextureInfoArced(pub Arc<TextureInfo>);
```

- `HaveID`：使用 `Arc<TextureInfo>` 的指针地址作为唯一 ID
- `Pipeline`：调用 `batch.set_texture(self.0.srv.clone(), self.0.width, self.0.height)`

#### Example

```rust
let tex_arced = TextureInfoArced(textures["player"].clone());

let obj = Sprite2DObject {
    spr: Sprite2D { /* ... */ },
    color: [1.0; 4],
    transform: Transform2D::IDENTITY,
    pipeline: tex_arced,      // 👈 这里使用
    layer: 0.0,
};
```

⚠️ **Warning**：`HaveID` 的实现基于指针地址。如果 `Arc` 被 clone 但指向同一个 `TextureInfo`，它们的 ID 相同（正确行为）。如果 `TextureInfo` 被释放后新分配的对象使用了同一块内存地址，理论上会导致 ID 冲突，但概率极低。

---

### 3.13 SpriteBatch2D & Pipeline Trait

**文件**：`graphic/d3d11/sprite_batch_2d.rs`  
**公开导出**：`krjw_engine::SpriteBatch2D`, `krjw_engine::graphic::d3d11::sprite_batch_2d::Pipeline`

基于 D3D11 的精灵批渲染器，使用 4 顶点 quad + 16-bit 索引。

#### Pipeline Trait

```rust
pub trait Pipeline: HaveID + Clone {
    fn apply_to_batch(&self, batch: &mut SpriteBatch2D);
}
```

`TextureInfoArced` 提供了 `Pipeline` 的标准实现。

#### 方法

```rust
impl SpriteBatch2D {
    pub fn new(device, capacity, vs, ps, input_layout) -> Result<Self>;
    pub fn set_texture(&mut self, srv, width, height);
    pub fn set_vertex_shader(&mut self, vs);
    pub fn set_pixel_shader(&mut self, ps);
    pub fn add(&mut self, pos: Vec2, scale: Vec2, rot: f32, sprite: &Sprite2D, color: [f32; 4]) -> Result<()>;
    pub fn set_mvp(&self, gfx: &D3D11, mvp: &glam::Mat4);
    pub fn submit_and_draw(&mut self, gfx: &D3D11) -> Result<()>;
    pub fn draw(&mut self, gfx: &D3D11, mvp: &glam::Mat4) -> Result<()>;
    pub fn clear_batch(&mut self);
    pub fn count(&self) -> usize;

    // 高级：将 Sprite2DBuffer 按 pipeline 排序后提交
    pub fn push_buffered<T, U>(
        &mut self,
        gfx: &D3D11,
        vp: &glam::Mat4,
        buf: &mut Sprite2DBuffer<T, U>,
        extract_transform: impl Fn(&U) -> (Vec2, Vec2, f32),
    ) where T: Pipeline, U: Clone;
}
```

#### Example — 标准用法

```rust
// 设置 MVP 矩阵
let vp = camera.vp_matrix();
batch.set_mvp(&gfx, &vp.transpose());

// 绑定纹理并添加精灵
batch.set_texture(tex_info.srv.clone(), tex_info.width, tex_info.height);
batch.add(
    Vec2::new(100.0, 200.0),   // 位置
    Vec2::new(2.0, 2.0),       // 缩放
    0.5,                        // 旋转
    &Sprite2D {
        origin_px: Vec2::new(16.0, 16.0),
        size_px: Vec2::new(32.0, 32.0),
        uv_tl_px: Vec2::ZERO,
        uv_size_px: Vec2::new(32.0, 32.0),
    },
    [1.0, 1.0, 1.0, 1.0],
)?;

// 提交绘制
batch.submit_and_draw(&gfx)?;
batch.clear_batch();
```

#### Example — push_buffered 高级用法

```rust
// 将 Sprite2DBuffer 中的精灵按 pipeline 分组自动提交
let vp = camera.vp_matrix();
batch.push_buffered(
    &gfx,
    &vp.transpose(),
    &mut sprite_buf,
    |t: &Transform2D| (t.pos, t.scale, t.rot),
);
```

⚠️ **Warning**：
- `capacity` 不能超过 `0xffff / 4 = 16383`（因为使用 16-bit 索引，每个 quad 用 4 个顶点）
- `set_mvp` 接收的矩阵需要是**转置后的**（因为 HLSL 使用列主序，而 glam 是行主序）
- `add()` 如果没有先调用 `set_texture()` 会 panic（`expect("No texture set")`）
- `push_buffered` 在每次 pipeline 切换时会调用 `submit_and_draw` + `clear_batch`，这意味着每切换一次就会产生一次 draw call

🚧 **TODO**：
- 可以考虑将 `MVP` 矩阵的转置放在 `SpriteBatch2D` 内部处理，减少用户心智负担
- 为 `push_buffered` 添加每个 pipeline 的最大精灵数限制（超过时自动拆分 draw call）
- 支持 instanced rendering 以提高性能

---

### 3.14 ShapeBatch2D

**文件**：`graphic/d3d11/shape_batch_2d.rs`  
**公开导出**：`krjw_engine::ShapeBatch2D`

形状批渲染器，用于绘制无纹理的线段、矩形、圆形、多边形。

#### 方法

```rust
impl ShapeBatch2D {
    pub fn new(device, capacity, vs, ps, input_layout) -> Result<Self>;

    // 无 UV 方法（适用于 ps_solid 纯色渲染，无需绑定纹理）
    pub fn add_rect_no_uv(&mut self, pos: Vec2, size: Vec2, rot: f32, color: [f32; 4]);
    pub fn add_circle_no_uv(&mut self, pos: Vec2, radius: f32, color: [f32; 4], segments: u32);
    pub fn add_square_line_no_uv(&mut self, from: Vec2, to: Vec2, thickness: f32, color: [f32; 4]);
    pub fn add_polygon_no_uv(&mut self, points: &[Vec2], color: [f32; 4]);

    // 带 UV 方法（需要通过 set_texture 绑定纹理）
    pub fn add_rect(&mut self, pos, size, rot, uv_tl_px, uv_size_px, color);
    pub fn add_circle(&mut self, pos, radius, uv_tl_px, uv_size_px, color, segments);

    // 通用
    pub fn push(&mut self, vertices: &[ShapeVertex], tri_indicies: &[[u16; 3]])；
    pub fn push_with_transform_2d(&mut self, vertices: &[ShapeVertex], tri_indicies: &[[u16; 3]], pos: Vec2, scale: Vec2, rot: f32)；
    pub fn set_texture(&mut self, srv, width, height);
    pub fn set_mvp(&self, gfx: &D3D11, mvp: &glam::Mat4);
    pub fn submit_and_draw(&mut self, gfx: &D3D11) -> Result<()>;
    pub fn draw(&mut self, gfx: &D3D11, mvp: &glam::Mat4) -> Result<()>;
    pub fn clear_batch(&mut self);
    pub fn count(&self) -> usize;
}
```

#### Example

```rust
// 绘制矩形边框
shape_batch.add_rect_no_uv(
    Vec2::new(100.0, 100.0),
    Vec2::new(50.0, 50.0),
    0.0,
    [1.0, 0.0, 0.0, 1.0],
);

// 绘制线段
shape_batch.add_square_line_no_uv(
    Vec2::new(0.0, 0.0),
    Vec2::new(200.0, 0.0),
    2.0,
    [0.0, 1.0, 0.0, 1.0],
);

// 绘制圆形
shape_batch.add_circle_no_uv(
    Vec2::new(300.0, 300.0),
    50.0,
    [0.0, 0.0, 1.0, 0.5],
    32,
);

// 绘制多边形（例如三角形）
shape_batch.add_polygon_no_uv(
    &[Vec2::new(400.0, 100.0), Vec2::new(450.0, 200.0), Vec2::new(350.0, 200.0)],
    [1.0, 1.0, 0.0, 0.8],
);

// 提交
let mvp = camera.vp_matrix().transpose();
shape_batch.draw(&gfx, &mvp)?;
shape_batch.clear_batch();
```

#### Example — 绘制碰撞体轮廓

```rust
fn draw_collider_outline(shape: &mut ShapeBatch2D, inst: &ColliderInstance, gfx: &D3D11, vp: &Mat4, color: [f32; 4]) {
    shape.set_mvp(gfx, vp);
    match inst.shape {
        Collider::AABB { half_size } | Collider::Rect { half_size } => {
            let h = *half_size * inst.xform.scale;
            shape.add_rect_no_uv(inst.xform.pos, h * 2.0, inst.xform.rot, color);
        }
        Collider::Circle { radius } => {
            let r = radius * inst.xform.scale.x.max(inst.xform.scale.y);
            shape.add_circle_no_uv(inst.xform.pos, r, color, 32);
        }
    }
}
```

⚠️ **Warning**：
- **性能问题**：`submit_and_draw` 实现中，每次提交都会**重建顶点缓冲区**（`write_buffer`），并且使用 `remap` 数组对顶点去重，这在大量三角形时开销较大
- 当三角形数量超过 `capacity` 时，会拆分为多个 draw call，每个 draw call 都重新映射顶点
- `add_rect_no_uv` 和 `add_rect` 的计算方式一致，但使用不同的顶点顺序（`add_square_line_no_uv` 也使用了不同的顶点生成方式）

🚧 **TODO**：
- 重构 `submit_and_draw` 的顶点提交策略，减少 CPU 端的 remap 开销
- 考虑使用预分配顶点缓冲区 + Map/Write/Discard 模式替代每次都创建临时 Vec

---

### 3.15 Transform2D

**文件**：`transform2d.rs`  
**公开导出**：`krjw_engine::Transform2D`

可组合的 2D 变换：旋转 → 缩放 → 平移（RST）。

#### 字段

```rust
pub struct Transform2D {
    pub pos: Vec2,    // 世界空间位置
    pub scale: Vec2,  // 局部缩放
    pub rot: f32,     // 旋转（弧度，逆时针）
}

impl Transform2D {
    pub const IDENTITY: Self;  // 单位变换
}
```

#### 方法

```rust
impl Transform2D {
    // Builder 模式
    pub fn with_pos(self, pos: Vec2) -> Self;
    pub fn with_scale(self, scale: Vec2) -> Self;
    pub fn with_rot(self, rot: f32) -> Self;
    pub fn move_by(self, pos: Vec2) -> Self;
    pub fn scale_by(self, scale: Vec2) -> Self;
    pub fn rotate_by(self, rot: f32) -> Self;

    // 组合与变换
    pub fn transform(&self, parent: &Transform2D) -> Self;  // 与父级组合
    pub fn transform_components(&self, pos: Vec2, scale: Vec2, rot: f32) -> Self;
    pub fn transform_point(&self, local_point: Vec2) -> Vec2;     // 局部→世界
    pub fn inverse_transform_point(&self, world_point: Vec2) -> Vec2;  // 世界→局部
}
```

#### 组合规则

对于子实体：
```
world = parent * self
```

数学表达式：
```
result.pos  = parent.pos + rotate(self.pos, parent.rot) * parent.scale
result.scale = self.scale * parent.scale
result.rot  = self.rot + parent.rot
```

#### Example

```rust
// 层级变换：角色 → 武器
let player_xform = Transform2D {
    pos: Vec2::new(100.0, 200.0),
    scale: Vec2::new(1.0, 1.0),
    rot: 0.5,  // 角色旋转
};

let weapon_local = Transform2D {
    pos: Vec2::new(30.0, 0.0),  // 武器相对角色偏移
    scale: Vec2::ONE,
    rot: 0.0,
};

let weapon_world = weapon_local.transform(&player_xform);
// weapon_world.pos = (100 + 30*cos(0.5), 200 + 30*sin(0.5))
```

```rust
// 点变换
let local_point = Vec2::new(10.0, 20.0);
let world_point = xform.transform_point(local_point);
let back_to_local = xform.inverse_transform_point(world_point);
```

---

### 3.16 Camera2D

**文件**：`camera2d.rs`  
**公开导出**：`krjw_engine::Camera2D`

正交 2D 相机，处理 View-Projection 矩阵和坐标转换。

#### 字段

```rust
pub struct Camera2D {
    pub position: Vec2,       // 世界空间中的相机位置
    pub rotation: f32,        // 相机旋转（弧度）
    pub zoom: Vec2,           // 缩放（Vec2 支持非均匀缩放）
    pub viewport_pos: Vec2,   // 视口左上角（窗口像素）
    pub viewport_size: Vec2,  // 视口尺寸（窗口像素）
}
```

#### 方法

```rust
impl Camera2D {
    pub fn new(window_size_px: Vec2) -> Self;  // 创建覆盖全窗口的相机
    pub fn move_by(&mut self, pos: Vec2);
    pub fn walk_xy(&mut self, xy: Vec2);              // 随旋转方向移动
    pub fn walk_xplus(&mut self, xplus: f32);         // 沿相机右方移动
    pub fn walk_yplus(&mut self, yplus: f32);         // 沿相机上方移动

    // 矩阵
    pub fn vp_matrix(&self) -> Mat4;     // 完整 View-Projection 矩阵
    pub fn view_matrix(&self) -> Mat4;
    pub fn projection_matrix(&self) -> Mat4;

    // 视口
    pub fn apply_viewport(&self, gfx: &D3D11);

    // 坐标转换
    pub fn screen_to_world(&self, screen_px: Vec2) -> Vec2;
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2;
}
```

#### Example

```rust
// 初始化
let mut camera = Camera2D::new(Vec2::new(960.0, 600.0));

// 每帧更新
// WASD 移动
let speed = 200.0 * dt;
if key_pressed!(driver, KeyCode::KeyW) {
    camera.walk_yplus(speed * dt);
}
if key_pressed!(driver, KeyCode::KeyS) {
    camera.walk_yplus(-speed * dt);
}
if key_pressed!(driver, KeyCode::KeyA) {
    camera.walk_xplus(-speed * dt);
}
if key_pressed!(driver, KeyCode::KeyD) {
    camera.walk_xplus(speed * dt);
}

// 滚轮缩放
let (_, wheel_y) = driver.mouse().get_mouse_wheel_delta();
camera.zoom *= 1.0 - (wheel_y as f32) * 0.1;
camera.zoom = camera.zoom.max(Vec2::splat(0.1)).min(Vec2::splat(10.0));

// 渲染时使用
let vp = camera.vp_matrix();
camera.apply_viewport(&gfx);
batch.set_mvp(&gfx, &vp.transpose());
// ... 绘制 ...

// 坐标转换
let world_click = camera.screen_to_world(mouse_pos);
let screen_enemy = camera.world_to_screen(enemy_pos);
```

⚠️ **Warning**：
- `apply_viewport` 直接与 D3D11 耦合（作者自己也吐槽过）
- `vp_matrix()` 返回的矩阵需要**转置**后才能传入 `set_mvp`（`batch.set_mvp(gfx, &vp.transpose())`）
- `new()` 假设窗口原点为 (0, 0)。如果你的窗口有菜单栏或工具栏偏移，需要手动设置 `viewport_pos`

🚧 **TODO**：
- 将 `apply_viewport` 抽象为平台无关的接口
- 提供正交投影的宽高比锁定选项

---

### 3.17 Collider / ColliderInstance / Overlap

**文件**：`collider.rs`  
**公开导出**：`krjw_engine::Collider`, `krjw_engine::ColliderInstance`, `krjw_engine::Overlap`

碰撞形状与碰撞检测系统。

#### Collider

```rust
pub enum Collider {
    AABB { half_size: Vec2 },     // 轴对齐包围盒（忽略旋转）
    Rect { half_size: Vec2 },     // 有向包围盒（应用完整变换）
    Circle { radius: f32 },       // 圆形（旋转无关）
}
```

#### ColliderInstance

```rust
pub struct ColliderInstance<'a> {
    pub shape: &'a Collider,
    pub xform: Transform2D,
}
```

#### Overlap

```rust
pub struct Overlap {
    pub hit: bool,       // 是否碰撞
    pub push: Vec2,      // 将自身推开的分辨向量
    pub normal: Vec2,    // 碰撞法线（从自身指向对方）
    pub depth: f32,      // 穿透深度
}
```

#### 方法

```rust
impl<'a> ColliderInstance<'a> {
    pub fn new(shape: &'a Collider, xform: Transform2D, parent: Option<&Transform2D>) -> Self;
    pub fn apply_transform(&self, parent: &Transform2D) -> Self;
    pub fn contains_point(&self, point: Vec2) -> bool;    // 点包含检测
    pub fn overlaps(&self, other: &ColliderInstance) -> Overlap;  // 碰撞检测
}
```

#### 碰撞配对算法

| 配对 | 算法 |
|------|------|
| AABB × AABB | 分离轴（简化为轴对齐比较） |
| AABB × Circle | 最近点法 |
| Circle × Circle | 球心距离 |
| 包含 Rect 的组合 | SAT（分离轴定理，最多测试 4 条轴） |

#### Example

```rust
let player_shape = Collider::Rect { half_size: Vec2::new(16.0, 16.0) };
let player_inst = ColliderInstance::new(
    &player_shape,
    Transform2D { pos: Vec2::new(100.0, 200.0), scale: Vec2::ONE, rot: 0.0 },
    None,
);

let wall_shape = Collider::AABB { half_size: Vec2::new(200.0, 20.0) };
let wall_inst = ColliderInstance::new(
    &wall_shape,
    Transform2D { pos: Vec2::new(0.0, 300.0), scale: Vec2::ONE, rot: 0.0 },
    None,
);

let overlap = player_inst.overlaps(&wall_inst);
if overlap.hit {
    println!("Collision! Push: {:?}, Normal: {:?}, Depth: {}", 
             overlap.push, overlap.normal, overlap.depth);
    // 使用 push 向量将玩家推开
    player.xform.pos += overlap.push;
}
```

```rust
// 点包含检测
let point = Vec2::new(105.0, 205.0);
if player_inst.contains_point(point) {
    println!("Point is inside player!");
}
```

#### Example — 层级碰撞检测

```rust
let parent = Transform2D {
    pos: Vec2::new(400.0, 300.0),
    scale: Vec2::new(2.0, 2.0),
    rot: 1.0,
};

let child = ColliderInstance::new(
    &Collider::Circle { radius: 10.0 },
    Transform2D { pos: Vec2::new(50.0, 0.0), scale: Vec2::ONE, rot: 0.0 },
    Some(&parent),  // 应用父级变换
);
```

⚠️ **Warning**：
- `ColliderInstance` 借用 `Collider`，需要注意生命周期管理
- AABB 模式**忽略旋转**：即使 `xform.rot` 非零，AABB 的检测仍然只用位置和缩放
- SAT 算法在大角度旋转的矩形对矩形时可能产生轻微的不精确，但在游戏场景中通常可以接受
- `Overlap::normal` 的方向总是从 `self` 指向 `other`（调用 `overlaps` 时的第一个对象是 `self`）

🚧 **TODO**：
- 添加射线检测（`ray_cast`）功能
- 添加胶囊体碰撞器支持
- 考虑引入宽阶段（Broad Phase）碰撞剔除（Spatial Hash / BVH）
- 支持连续的碰撞穿透修复（Continuous Collision Detection）

---

### 3.18 AtlasText & TextLayout

**文件**：`atlas_text.rs`  
**公开导出**：`krjw_engine::AtlasText`, `krjw_engine::TextLayout`

动态文字图集系统。使用 `cosmic-text` 排版，`swash` 直接光栅化，自定义 Skyline Packer 打包到 2048×2048 的图集页中。

#### TextLayout

```rust
pub struct TextLayout {
    pub(crate) glyphs: Vec<(cosmic_text::CacheKey, Vec2)>,
}
```

存储排版结果，可以重复使用以在不同变换/颜色下渲染相同文字（例如绘制阴影效果）。

#### AtlasText

```rust
pub struct AtlasText {
    // ...
    pub lifetime_a: f32,  // 字形缓存的寿命参数 a
    pub lifetime_b: f32,  // 字形缓存的寿命参数 b
}
```

#### 方法

```rust
impl AtlasText {
    pub fn new(device: &ID3D11Device, lifetime_a: f32, lifetime_b: f32) -> Result<Self>;

    // 信息
    pub fn page_count(&self) -> usize;
    pub fn texture_info(&self, page_idx: usize) -> &TextureInfo;
    pub fn texture_arced(&self, page_idx: usize) -> TextureInfoArced;
    pub fn set_lifetime_params(&mut self, a: f32, b: f32);

    // 排版与渲染
    pub fn layout_text(&mut self, text: &str, metrics: Metrics, attrs: Attrs,
                       shaping: Shaping, device: &ID3D11Device) -> Result<TextLayout>;
    pub fn render_layout(&self, layout: &TextLayout, offset: Vec2, origin: Vec2,
                         transform: Transform2D, color: [f32; 4], layer: f64,
                         buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>);
    pub fn render_layout_simple(&self, layout: &TextLayout, offset: Vec2,
                                color: [f32; 4], layer: f64,
                                buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>);
    pub fn render_text(&mut self, text: &str, metrics: Metrics, attrs: Attrs,
                       shaping: Shaping, offset: Vec2, color: [f32; 4], layer: f64,
                       buffer: &mut Sprite2DBuffer<TextureInfoArced, Transform2D>,
                       device: &ID3D11Device) -> Result<()>;

    // GPU 上传（每帧需调用）
    pub fn upload(&mut self, gfx: &D3D11) -> Result<()>;
    pub fn clear(&mut self, device: &ID3D11Device) -> Result<()>;
}
```

#### 字形缓存寿命

寿命公式：`lifetime = max(side * lifetime_a + lifetime_b, 1)`（帧数）
- `side` = 字形的最大边长
- `lifetime_a`：基于大小的系数（大字形活更久）
- `lifetime_b`：固定寿命偏移

#### Example

```rust
// 初始化
let mut atlas = AtlasText::new(&gfx.device, 0.5, 60.0)?; // 小字形 60 帧，大字形更久

// 每帧流程
// 1. 排版（首次或文字变化时）
let layout = atlas.layout_text(
    "Hello, World!",
    Metrics::new(14.0, 20.0),     // font_size, line_height
    Attrs::new().color(Color::rgb(255, 255, 255)),
    Shaping::Advanced,
    &gfx.device,
)?;

// 2. 渲染到精灵缓冲区
// 简单渲染
atlas.render_layout_simple(
    &layout,
    Vec2::new(10.0, 10.0),
    [1.0, 1.0, 1.0, 1.0],
    100.0,
    &mut text_buf,
);

// 高级渲染（带阴影）
// 先渲染阴影（偏移 + 半透明）
atlas.render_layout(
    &layout,
    Vec2::new(12.0, 12.0),    // 偏移 2px
    Vec2::ZERO,
    Transform2D::IDENTITY,
    [0.0, 0.0, 0.0, 0.5],    // 半透明黑色阴影
    99.0,                      // 在文字之前绘制
    &mut text_buf,
);
// 再渲染文字本身
atlas.render_layout(
    &layout,
    Vec2::new(10.0, 10.0),
    Vec2::ZERO,
    Transform2D::IDENTITY,
    [1.0, 1.0, 1.0, 1.0],
    100.0,
    &mut text_buf,
);

// 3. 上传图集脏页到 GPU
atlas.upload(&gfx)?;

// 4. 渲染精灵缓冲区（通过 SpriteBatch2D 提交）
let vp = camera.vp_matrix();
batch.push_buffered(&gfx, &vp.transpose(), &mut text_buf, |t| (t.pos, t.scale, t.rot));
```

#### Example — 一次性渲染

```rust
atlas.render_text(
    "One-shot text",
    Metrics::new(24.0, 32.0),
    Attrs::new().color(Color::rgb(255, 0, 0)),
    Shaping::Advanced,
    Vec2::new(100.0, 200.0),
    [1.0, 0.0, 0.0, 1.0],
    50.0,
    &mut text_buf,
    &gfx.device,
)?;
```

⚠️ **Warning**：
- **图集页不可回收**：当字形被逐出（evict）后，其在 atlas 中占用的空间无法回收利用。SkylinePacker 只有 `allocate` 没有 `deallocate`。长时间运行可能导致图集页用满。
- 每个 atlas 页为 2048×2048，每页像素数据占用约 16MB RAM + GPU 显存
- 每帧调用 `upload()` 时会**整页上传**，即使只更改了几个像素
- `layout_text()` 中每个 glyph 都会独立渲染和上传，大规模文字可能产生较多 draw call
- 没有内置字体管理——所有字体通过 `FontSystem` 自动加载系统字体，无法指定自定义字体文件路径

🚧 **TODO**：
- **图集空间回收**：SkylinePacker 需要支持移除已释放的段（segment），或者整个页面可以重置
- **部分上传**：只上传脏区域而非整页
- **自定义字体**：允许加载 `ttf`/`otf` 文件作为字体源
- **特殊字符支持**：Emoji 渲染（当前 swash 配置中 `Source::ColorBitmap` 可能不完整）
- `TextLayout` 目前没有提供测量文本尺寸的方法（例如 `text_bounds()`）
- 没有提供文字对齐（左/中/右）的内置功能

---

## 4. 完整示例

以下是一个综合示例，展示如何使用引擎的大部分核心功能。

### 4.1 完整 App 骨架

**`my_app/src/main.rs`**
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

**`my_app/src/app.rs`**
```rust
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use anyhow::Result;
use glam::Vec2;
use image::io::Reader as ImageReader;
use krjw_engine::*;
use cosmic_text::{Attrs, Metrics, Shaping};
use winit::keyboard::KeyCode;

pub struct App {
    pub ctx: Option<AppContext>,
}

pub struct AppContext {
    pub window: winit::window::Window,
    pub gfx: D3D11,
    pub batch: SpriteBatch2D,
    pub shape_batch: ShapeBatch2D,
    pub textures: std::collections::HashMap<String, Arc<TextureInfo>>,
    pub camera: Camera2D,
    pub timer: Timer,
    pub sprite_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    pub text_buf: Sprite2DBuffer<TextureInfoArced, Transform2D>,
    pub atlas: AtlasText,
    pub player_pos: Vec2,
    pub player_rot: f32,
}

impl Default for App {
    fn default() -> Self {
        App { ctx: None }
    }
}

impl App {
    pub fn run(&mut self, window: winit::window::Window, hwnd: isize,
               rx: Receiver<AppMsg>) -> Result<()> {
        // ── D3D11 初始化 ──
        let gfx = D3D11::init_on_hwnd(hwnd)?;
        let size = window.inner_size();
        let window_size = Vec2::new(size.width as f32, size.height as f32);

        // ── 事件驱动 ──
        let mut driver = EventDriver::new(rx);
        driver.set_initial_window_size(size.width, size.height);

        // ── 批渲染器 ──
        let batch = SpriteBatch2D::new(&gfx.device, 2048,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_tex_rgba_2d,
            &gfx.states.input_layout_puc)?;
        let shape_batch = ShapeBatch2D::new(&gfx.device, 4096,
            &gfx.states.vs_puc_m_2d, &gfx.states.ps_solid_2d,
            &gfx.states.input_layout_puc)?;

        // ── 纹理加载 ──
        let mut textures = std::collections::HashMap::new();
        if let Ok(img) = ImageReader::open("assets/player.png") {
            if let Ok(img) = img.decode() {
                let tex = d3d11_utils::load_texture_from_dynamic_image(&gfx.device, &img)?;
                textures.insert("player".to_string(), Arc::new(tex));
            }
        }

        // ── 相机 ──
        let camera = Camera2D::new(window_size);

        // ── 文字图集 ──
        let atlas = AtlasText::new(&gfx.device, 0.5, 60.0)?;

        let ctx = AppContext {
            window, gfx, batch, shape_batch, textures,
            camera, timer: Timer::default(),
            sprite_buf: Sprite2DBuffer::default(),
            text_buf: Sprite2DBuffer::default(),
            atlas, player_pos: Vec2::ZERO, player_rot: 0.0,
        };
        self.ctx = Some(ctx);
        self.main_loop(&mut driver)
    }

    fn main_loop(&mut self, driver: &mut EventDriver) -> Result<()> {
        let ctx = self.ctx.as_mut().unwrap();

        loop {
            // ── 1. 事件处理 ──
            let events = driver.poll_frame();
            if events.close_requested || events.disconnected { break; }

            // 窗口大小变化
            if driver.window_size_dirty() {
                let (w, h) = driver.window_size();
                ctx.gfx.on_resize(w, h)?;
                ctx.camera.viewport_size = Vec2::new(w as f32, h as f32);
                driver.clear_window_size_dirty();
            }

            // ── 2. Delta Time ──
            let dt = ctx.timer.pre_frame_and_get_delta_time().min(0.05);

            // ── 3. 更新逻辑 ──
            self.update(ctx, driver, dt)?;

            // ── 4. 渲染 ──
            self.render(ctx, dt)?;

            // ── 5. Present ──
            ctx.gfx.present()?;
            ctx.timer.post_frame_fpsc(dt);
            driver.end_frame();
        }
        Ok(())
    }

    fn update(&mut self, ctx: &mut AppContext, driver: &EventDriver, dt: f64) -> Result<()> {
        // ── 键盘控制玩家 ──
        let speed = 200.0;
        if driver.keyboard().get_key_state(KeyCode::KeyW).is_pressed() {
            ctx.player_pos.y -= speed * dt as f32;
        }
        if driver.keyboard().get_key_state(KeyCode::KeyS).is_pressed() {
            ctx.player_pos.y += speed * dt as f32;
        }
        if driver.keyboard().get_key_state(KeyCode::KeyA).is_pressed() {
            ctx.player_pos.x -= speed * dt as f32;
        }
        if driver.keyboard().get_key_state(KeyCode::KeyD).is_pressed() {
            ctx.player_pos.x += speed * dt as f32;
        }

        // 旋转
        if driver.keyboard().get_key_state(KeyCode::KeyQ).is_pressed() {
            ctx.player_rot -= 2.0 * dt as f32;
        }
        if driver.keyboard().get_key_state(KeyCode::KeyE).is_pressed() {
            ctx.player_rot += 2.0 * dt as f32;
        }

        // ── 相机控制 ──
        let cam_speed = 300.0;
        if driver.keyboard().get_key_state(KeyCode::ArrowUp).is_pressed() {
            ctx.camera.walk_yplus(cam_speed * dt as f32);
        }
        if driver.keyboard().get_key_state(KeyCode::ArrowDown).is_pressed() {
            ctx.camera.walk_yplus(-cam_speed * dt as f32);
        }
        if driver.keyboard().get_key_state(KeyCode::ArrowLeft).is_pressed() {
            ctx.camera.walk_xplus(-cam_speed * dt as f32);
        }
        if driver.keyboard().get_key_state(KeyCode::ArrowRight).is_pressed() {
            ctx.camera.walk_xplus(cam_speed * dt as f32);
        }

        // 鼠标滚轮缩放
        if let Some((_, wheel_y)) = driver.mouse().get_pixel_wheel() {
            ctx.camera.zoom *= 1.0 - (wheel_y as f32) * 0.001;
            ctx.camera.zoom = ctx.camera.zoom.max(Vec2::splat(0.1)).min(Vec2::splat(10.0));
        } else {
            let (_, wheel_y) = driver.mouse().get_mouse_wheel_delta();
            ctx.camera.zoom *= 1.0 - (wheel_y as f32) * 0.1;
            ctx.camera.zoom = ctx.camera.zoom.max(Vec2::splat(0.1)).min(Vec2::splat(10.0));
        }

        Ok(())
    }

    fn render(&mut self, ctx: &mut AppContext, dt: f64) -> Result<()> {
        // ── 清空缓冲区 ──
        ctx.sprite_buf.clear();
        ctx.text_buf.clear();
        ctx.shape_batch.clear_batch();
        ctx.gfx.clear_screen(&[0.1, 0.1, 0.2, 1.0]);

        // ── 渲染精灵 ──
        if let Some(tex) = ctx.textures.get("player") {
            let tex_arced = TextureInfoArced(tex.clone());
            let sprite_obj = Sprite2DObject {
                spr: Sprite2D {
                    origin_px: Vec2::new(32.0, 32.0),  // 中心原点
                    size_px: Vec2::new(64.0, 64.0),
                    uv_tl_px: Vec2::ZERO,
                    uv_size_px: Vec2::new(64.0, 64.0),
                },
                color: [1.0, 1.0, 1.0, 1.0],
                transform: Transform2D {
                    pos: ctx.player_pos,
                    scale: Vec2::ONE,
                    rot: ctx.player_rot,
                },
                pipeline: tex_arced,
                layer: 10.0,
            };
            ctx.sprite_buf.push(&sprite_obj);
        }

        // ── 渲染文字 HUD ──
        let fps_text = format!("FPS: {:.1}", ctx.timer.get_fps());
        let layout = ctx.atlas.layout_text(
            &fps_text,
            Metrics::new(14.0, 20.0),
            Attrs::new(),
            cosmic_text::Shaping::Advanced,
            &ctx.gfx.device,
        )?;

        // 文字阴影
        ctx.atlas.render_layout(
            &layout,
            Vec2::new(12.0, 12.0),
            Vec2::ZERO,
            Transform2D::IDENTITY,
            [0.0, 0.0, 0.0, 0.5],
            99.0,
            &mut ctx.text_buf,
        );
        // 文字本体
        ctx.atlas.render_layout(
            &layout,
            Vec2::new(10.0, 10.0),
            Vec2::ZERO,
            Transform2D::IDENTITY,
            [1.0, 1.0, 0.0, 1.0],  // 黄色
            100.0,
            &mut ctx.text_buf,
        );

        // ── 渲染碰撞体（调试用） ──
        let player_collider = Collider::Rect { half_size: Vec2::new(32.0, 32.0) };
        let inst = ColliderInstance::new(
            &player_collider,
            Transform2D { pos: ctx.player_pos, scale: Vec2::ONE, rot: ctx.player_rot },
            None,
        );
        match inst.shape {
            Collider::Rect { half_size } => {
                let h = *half_size * inst.xform.scale;
                ctx.shape_batch.add_rect_no_uv(
                    inst.xform.pos, h * 2.0, inst.xform.rot,
                    [0.0, 1.0, 0.0, 0.8],
                );
            }
            _ => {}
        }

        // ── 提交绘制 ──
        let vp = ctx.camera.vp_matrix().transpose();

        // 精灵和文字使用 SpriteBatch2D
        ctx.batch.push_buffered(&ctx.gfx, &vp, &mut ctx.sprite_buf, |t| (t.pos, t.scale, t.rot));
        // 上传图集纹理
        ctx.atlas.upload(&ctx.gfx)?;
        // 渲染精灵
        ctx.batch.submit_and_draw(&ctx.gfx);

        // 形状使用 ShapeBatch2D
        ctx.shape_batch.draw(&ctx.gfx, &vp)?;

        Ok(())
    }
}
```

---

## 5. 附录：已知问题与未来规划

### 5.1 已知问题（⚠️ Warning 汇总）

| 问题 | 模块 | 影响 |
|------|------|------|
| 仅支持 Windows x64 | 全局 | 无法在非 Windows 平台运行 |
| 硬编码窗口标题 "KrisuRJW" | EngineHandler | 用户无法自定义窗口标题 |
| `create_texture_2d` 类型不安全 | d3d11_utils | `cpu_access_flags` 使用 `u32` 非强类型枚举 |
| RGB8 图片自动扩展至 RGBA8 | d3d11_utils | 额外的内存和性能开销 |
| 无 MipMap 支持 | d3d11_utils | 缩放时纹理质量下降 |
| `present` 固定 v-sync | D3D11 | 无法关闭垂直同步 |
| Debug 模式需要 D3D Debug Layer | D3D11 | 没有安装 Debug Layer 时会报错 |
| `ShapeBatch2D::submit_and_draw` 每次重建 VB | ShapeBatch2D | 大量三角形时性能下降 |
| AtlasText 图集空间不可回收 | AtlasText | 长时间运行可能耗尽图集页 |
| AtlasText 整页上传 | AtlasText | 小量文本变化也会上传整页 |
| `MouseInput::device_event` 中滚轮事件注释 | MouseInput | 设备事件的滚轮检测不可用 |
| 无资源管理器 | 全局 | 用户需手动维护纹理 HashMap |

### 5.2 未来规划（🚧 TODO 汇总）

- [ ] 跨平台支持（Vulkan / Metal 后端）
- [ ] 可自定义的窗口属性（标题、尺寸、模式、图标）
- [ ] MipMap 生成
- [ ] 从文件路径直接加载纹理
- [ ] 可配置的 v-sync（`SyncInterval` 参数化）
- [ ] 形状批渲染器性能优化（Map/Discard 代替重建）
- [ ] AtlasText 图集空间回收机制
- [ ] AtlasText 部分上传（只有脏区域）
- [ ] AtlasText 自定义字体加载
- [ ] AtlasText 文本测量和对齐功能
- [ ] 碰撞检测的射线检测（Ray Cast）
- [ ] 宽阶段碰撞剔除（Spatial Hash）
- [ ] 统一的资源管理器/加载器
- [ ] 可选的深度测试（纯 2D 场景不需要）
- [ ] `EngineHandler` 提供更优雅的关闭流程
- [ ] 示例项目中的宏提取到引擎中
- [ ] 内置 ECS 支持（可选）

---

> **作者**：难以置信，这样真的可以跑  
> 如果发现任何 bug 或有改进建议，欢迎提交 Issue 或 PR。