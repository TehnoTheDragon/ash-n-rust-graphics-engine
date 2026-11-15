use std::sync::Arc;

use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::EventLoop, window::{Window, WindowButtons}};

use crate::gpu::renderer::Renderer;

mod gpu;

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_none() {
            let window = event_loop.create_window(winit::window::WindowAttributes::default()
                .with_inner_size(winit::dpi::PhysicalSize::new(800, 600))
                .with_title("Vulky... whatever")
            ).expect("Could not create window");
            let window = Arc::new(window);
            let renderer = Renderer::new(window.clone()).expect("Could not create renderer");

            self.window = Some(window);
            self.renderer = Some(renderer);
        }
    }

     fn window_event(
         &mut self,
         event_loop: &winit::event_loop::ActiveEventLoop,
         window_id: winit::window::WindowId,
         event: winit::event::WindowEvent,
     )
     {
         match event {
            WindowEvent::CloseRequested => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.destroy().expect("Could not destroy renderer");
                }
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.render().unwrap();
                }
                if let Some(ref window) = self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height).expect("Could not resize swapchain");
                }
            }
            _ => {}
         }
     }
}

fn main() -> anyhow::Result<()> {
    let mut app = App::default();
    let event_loop = EventLoop::builder().build()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}