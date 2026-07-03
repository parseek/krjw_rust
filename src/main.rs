use winit::event_loop::ControlFlow;

mod app;

fn main() {
    println!("RS260701 by KrisuRJW");

    let event_loop = winit::event_loop::EventLoop::new()
        .unwrap_or_else(|e| panic!("Failed to create event loop: {}", e));
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = app::App::default();
    event_loop
        .run_app(&mut app)
        .unwrap_or_else(|e| panic!("Failed to run event loop: {}", e));
}
