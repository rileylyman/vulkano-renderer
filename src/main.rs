#[macro_use]
extern crate vulkano;
extern crate vulkano_shaders;
extern crate winit;
extern crate vulkano_win;

use std::cmp::{min, max};
use std::vec::Vec;
use std::time::Instant;
use std::sync::Arc;
use vulkano::instance::{PhysicalDevice, Instance};
use vulkano_win::VkSurfaceBuild;
use winit::{Event, WindowEvent, WindowBuilder, EventsLoop, Window};
use vulkano::device::{Device, DeviceExtensions};
use vulkano::swapchain::{AcquireError, Swapchain, SwapchainCreationError, PresentMode};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::image::swapchain::SwapchainImage;
use vulkano::buffer::{CpuBufferPool, BufferUsage, CpuAccessibleBuffer};
use vulkano::pipeline::{viewport::Viewport, GraphicsPipeline};
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass};
use vulkano::format::ClearValue;
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::sync;
use vulkano::sync::{FlushError, GpuFuture};

#[derive(Clone, Debug)]
struct Vertex {
    position: [f32;2]
} vulkano::impl_vertex!(Vertex, position);

fn main() {
    let instance = {
        let extensions = vulkano_win::required_extensions();
        let info = app_info_from_cargo_toml!();
        Instance::new(Some(&info), &extensions, None).expect("Could not create instance")
    };

    //TODO: Filter devices by
    //  1.) Optional features needed by my application
    //  2.) Devices that can draw to my surface
    //  3.) Let user choose between the rest (or just choose first one after that)
    let physical_device = PhysicalDevice::enumerate(&instance).next().expect("No devices");

    let mut events_loop = EventsLoop::new();
    let surface = WindowBuilder::new()
        .with_title("Riley's Vulkan Render Engine")
        .with_decorations(true)
        .build_vk_surface(&events_loop, instance.clone())
        .expect("Could not create window");
    let window = surface.window();

    let queue_family = physical_device.queue_families().find(|&q| {
        q.supports_graphics() && surface.is_supported(q).unwrap_or(false)
    }).expect("Could not find queue.");
    
    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        .. DeviceExtensions::none()
    };
    let (device, mut queues) = Device::new(physical_device, physical_device.supported_features(),
        &device_extensions, [(queue_family, 0.5)].iter().cloned()).expect("Could not create device");

    //TODO: Use multiple queues, and more efficiently.
    let queue = queues.next().expect("Could not retrieve queue from queues");

    let (mut swapchain, images) = {
        let capabilities = surface.capabilities(physical_device).unwrap();

        let usage = capabilities.supported_usage_flags;

        let alpha = capabilities.supported_composite_alpha.iter().next().unwrap();
        
        //TODO: Choose format based on our needs.
        let format = capabilities.supported_formats[0].0;
        
        //TODO: Use more layers if necessary.
        let layers = 1;

        //NOTE: We could set this to capabilities.current_extent.unwrap_or(DEFAULT..)
        //But since either way we want the initial dimensions to be the window dimensions
        //we just get the physical dimensions this way
        let dimensions = if let Some(dimensions) = window.get_inner_size() {
            let dims: (u32, u32) = dimensions.to_physical(window.get_hidpi_factor()).into();
            [dims.0, dims.1]
        } else {
            return;
        };

        let transform = capabilities.current_transform;

        //Attempt to use triple buffering
        let buffer_count = if let Some(limit) = capabilities.max_image_count {
            min(3, limit)
        } else { 
            max(3, capabilities.min_image_count) 
        };

        let clip = true; //Clip parts of the buffer which aren't visible

        let present_mode = PresentMode::Fifo;

        Swapchain::new(device.clone(), surface.clone(), buffer_count, format, dimensions,
            layers, usage, &queue, transform, alpha, present_mode, clip, None)
            .expect("Could not create Swapchain")
    };

    let vertex_buffer = CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(), [
        Vertex { position: [-0.5, -0.25] },
        Vertex { position: [0.0, 0.5] },
        Vertex { position: [0.25, -0.1] },
    ].iter().cloned()).expect("Could not create vertex buffer");

    //Ring buffer than contains sub-buffers which are freed upon being dropped (cleanup_finished())
    let fragment_color_buffer = CpuBufferPool::<frag::ty::ColorData>::new(device.clone(), BufferUsage::all());

    let vs = vertex::Shader::load(device.clone()).expect("Could not load vertex shader");
    let fs =   frag::Shader::load(device.clone()).expect("Could not load fragment shader");

    let render_pass = Arc::new(vulkano::single_pass_renderpass!(
        device.clone(),
        attachments: {
            color: {
                load: Clear,
                store: Store,
                format: swapchain.format(),
                samples: 1,
            }
        },
        pass: {
            color: [color],
            depth_stencil: {}
        }
    ).expect("Could not create renderpass"));
    
    let pipeline = Arc::new(GraphicsPipeline::start()
        .vertex_input_single_buffer::<Vertex>()
        .vertex_shader(vs.main_entry_point(), ())
        .triangle_list()
        .viewports_dynamic_scissors_irrelevant(1)
        .fragment_shader(fs.main_entry_point(), ())
        .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
        .build(device.clone())
        .expect("Could not create GraphicsPipeline"));
    
    let mut dynamic_state = DynamicState { line_width: None, viewports: None, scissors: None };

    let mut framebuffers = gen_framebuffers_from_window_size(&images, render_pass.clone(), &mut dynamic_state);

    let mut recreate_swapchain = false;

    let mut previous_frame_end = Box::new(sync::now(device.clone())) as Box<GpuFuture>;

    let mut done = false;

    let start = Instant::now();

    loop {
        previous_frame_end.cleanup_finished();
        if recreate_swapchain {
           
            let dimensions = get_window_dimensions(&window).expect("Could not get new window dimensions");
            let (new_swapchain, new_images) = match swapchain.recreate_with_dimension(dimensions) {
               Ok(res) => res,
               Err(SwapchainCreationError::UnsupportedDimensions) => continue,
               Err(err) => panic!("{:?}", err)
            }; 

            swapchain = new_swapchain;
            framebuffers = gen_framebuffers_from_window_size(&new_images, render_pass.clone(), &mut dynamic_state);
            recreate_swapchain = false;
        } 

        let (image_num, acquire_future) = match vulkano::swapchain::acquire_next_image(swapchain.clone(), None) {
            Ok(res) => res,
            Err(AcquireError::OutOfDate) => {
                recreate_swapchain = true;
                continue;
            }, 
            Err(err) => panic!("{:?}", err)
        }; 

        let clear_values: Vec<ClearValue> = vec!([0.0, 0.3, 0.6, 1.0].into());

        let fragment_color_subbuffer = {
            let elapsed = (start.elapsed().as_millis() % 1000) as f32 / 1000.0;           
            let data = frag::ty::ColorData {
                color_data: [elapsed, 0.5, 0.5].into()
            };  
            fragment_color_buffer.next(data).expect("Could not generate next triangle color")
        }; 

        let set = Arc::new(PersistentDescriptorSet::start(pipeline.clone(), 0)
            .add_buffer(fragment_color_subbuffer).expect("Could not add fragment subbuffer to descriptor set")
            .build().unwrap());

        let command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family()).unwrap()
            .begin_render_pass(framebuffers[image_num].clone(), false, clear_values).unwrap()
            .draw(pipeline.clone(), &dynamic_state, vertex_buffer.clone(), set, ()).unwrap()
            .end_render_pass().unwrap()
            .build().unwrap();
        
        let future = previous_frame_end.join(acquire_future)
            .then_execute(queue.clone(), command_buffer).expect("Failure executing command buffer")
            .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => previous_frame_end = Box::new(future) as Box<_>,
            Err(FlushError::OutOfDate) => {
                recreate_swapchain = true;
                previous_frame_end = Box::new(sync::now(device.clone())) as Box<_>;
            }
            Err(err) => {
                println!("{:?}", err);
                previous_frame_end = Box::new(sync::now(device.clone())) as Box<_>;
            }  
        } 

        events_loop.poll_events(|event| {
            match event {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => done = true,
                Event::WindowEvent { event: WindowEvent::Resized(_), .. } => recreate_swapchain = true,
                _ => ()
            }  
        }); 
        if done { return; } 

    }

}


fn gen_framebuffers_from_window_size(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPassAbstract + Send + Sync>,
    dynamic_state: &mut DynamicState
    ) -> Vec<Arc<FramebufferAbstract + Send + Sync>> {

    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0
    };
    dynamic_state.viewports = Some(vec!(viewport));

    images.iter().map(|image| {
        Arc::new(
            Framebuffer::start(render_pass.clone())
                .add(image.clone()).unwrap()
                .build().unwrap()
        ) as Arc<FramebufferAbstract + Send + Sync>    
    }).collect::<Vec<_>>()
}

fn get_window_dimensions(window: &Window) -> Result<[u32;2], ()> {
    if let Some(dimensions) = window.get_inner_size() {
        let dimensions: (u32, u32) = dimensions.to_physical(window.get_hidpi_factor()).into();
        Ok([dimensions.0, dimensions.1])
    } else {
       Err(()) 
    }  
}  

mod vertex {
    vulkano_shaders::shader!{
        ty: "vertex",
        path: "src/vertex.glsl"
    }
}

mod frag {
    vulkano_shaders::shader!{
        ty: "fragment",
        path: "src/frag.glsl"
    }
}

