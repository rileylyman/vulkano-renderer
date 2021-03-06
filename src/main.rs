#[macro_use]
extern crate vulkano;
extern crate vulkano_shaders;
extern crate winit;
extern crate vulkano_win;
extern crate image;

use std::cmp::{min, max};
use std::vec::Vec;
use std::time::Instant;
use std::sync::Arc;
use std::error::Error;
use winit::{Event, WindowEvent, WindowBuilder, EventsLoop, Window};
use image::ImageFormat;
use vulkano::instance::{PhysicalDevice, Instance};
use vulkano::sampler::{Sampler, Filter, MipmapMode, SamplerAddressMode};
use vulkano::image::{ImageCreationError, immutable::ImmutableImage, Dimensions};
use vulkano_win::VkSurfaceBuild;
use vulkano::device::{Device, DeviceExtensions, Queue, QueuesIter};
use vulkano::swapchain::{AcquireError, Surface, Swapchain, SwapchainCreationError, PresentMode};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::image::{AttachmentImage, swapchain::SwapchainImage};
use vulkano::buffer::{CpuBufferPool, BufferUsage, CpuAccessibleBuffer};
use vulkano::pipeline::{GraphicsPipelineAbstract, viewport::Viewport, vertex::TwoBuffersDefinition, GraphicsPipeline};
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract, Subpass};
use vulkano::format::{Format, ClearValue};
use vulkano::command_buffer::{CommandBufferExecFuture, AutoCommandBuffer, AutoCommandBufferBuilder, DynamicState};
use vulkano::sync;
use vulkano::sync::{NowFuture, FlushError, GpuFuture};

mod objload;
mod teapot;

#[derive(Clone, Debug)]
pub struct Vertex {
    position: (f32, f32, f32),
} vulkano::impl_vertex!(Vertex, position);

#[derive(Clone, Debug)]
pub struct Normal {
    normal: (f32, f32, f32),
} vulkano::impl_vertex!(Normal, normal);

#[derive(Clone, Debug)]
pub struct TexVert {
    position2D: (f32, f32),
} vulkano::impl_vertex!(TexVert, position2D); 

pub type IndexType = u16;
#[derive(Clone, Debug)]
pub struct Indices {
    v: Vec<IndexType>,
    vn: Vec<IndexType>,
    vt: Vec<IndexType>,
}  

fn main() {

    let (device, mut queues, surface, mut events_loop) = init_vulkan().expect("Intialization error");
    let window = surface.window();

    //TODO: Use multiple queues, and more efficiently.
    let queue = queues.next().expect("Could not retrieve queue from queues");

    let (mut swapchain, images) = gen_swapchain(surface.clone(), queue.clone(), device.clone())
        .expect("Could not create swapchain");

    //let (vertices, tex_verts, normals, indices) = objload::load_model(include_str!("res/chalet.obj"))
    //    .expect("Could not load model");

    let vertices = teapot::VERTICES;
    let normals = teapot::NORMALS;
    let indices = teapot::INDICES;

    let vertex_buffer = CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(), 
        vertices.iter().cloned()).expect("Could not create vertex buffer");
    let normals_buffer = CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(), 
        normals.iter().cloned()).expect("Could not create vertex buffer");
    let v_index_buffer = CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(),
        indices.iter().cloned()).expect("Could not create v_index buffer");
    //let vn_index_buffer = CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(),
    //    indices.vn.iter().cloned()).expect("Could not create vt_index buffer");

    //Ring buffer that contains sub-buffers which are freed upon being dropped (cleanup_finished())
    //let fragment_color_buffer = CpuBufferPool::<frag::ty::ColorData>::new(device.clone(), BufferUsage::all());

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
            },
            depth: {
               load: Clear,
               store: DontCare,
               format: Format::D16Unorm,
               samples: 1,
            } 
        },
        pass: {
            color: [color],
            depth_stencil: {depth}
        }
    ).expect("Could not create renderpass"));
    
    //let (texture, texture_future) = load_texture(queue.clone(), include_bytes!("res/texture.png"))
    //    .expect("Error loading texture");
    //let sampler = Sampler::new(device.clone(), Filter::Linear, Filter::Linear,
    //    MipmapMode::Nearest, SamplerAddressMode::Repeat, SamplerAddressMode::Repeat,
    //    SamplerAddressMode::Repeat, 0.0, 1.0, 0.0, 0.0).expect("Could not create sampler");
    //let texture_set = Arc::new(PersistentDescriptorSet::start(pipeline.clone(), 1)
    //    .add_sampled_image(texture.clone(), sampler.clone())
    //    .expect("Could not add sampled image")
    //    .build().unwrap());
   

    let (mut pipeline, mut framebuffers) = gen_framebuffers_from_window_size(
        &images, render_pass.clone(), device.clone(), &vs, &fs);
    let mut recreate_swapchain = false;
    //let mut previous_frame_end = Box::new(texture_future) as Box<GpuFuture>;
    let mut previous_frame_end = Box::new(sync::now(device.clone())) as Box<GpuFuture>;
    let mut done = false;

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
            let (new_pipeline, new_framebuffers) = gen_framebuffers_from_window_size(&new_images, 
                render_pass.clone(), device.clone(), &vs, &fs);
            
            pipeline = new_pipeline;
            framebuffers = new_framebuffers;

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

        let clear_values = [0.0, 0.3, 0.6, 1.0];

        //let fragment_color_subbuffer = {
        //    let elapsed = (start.elapsed().as_millis() % 1000) as f32 / 1000.0;           
        //    let data = frag::ty::ColorData {
        //        color_data: [elapsed, 0.5, 0.5].into()
        //    };  
        //    fragment_color_buffer.next(data).expect("Could not generate next triangle color")
        //}; 

        //let set = Arc::new(PersistentDescriptorSet::start(pipeline.clone(), 0)
        //    .add_buffer(fragment_color_subbuffer).expect("Could not add fragment subbuffer to descriptor set")
        //    .build().unwrap());

        let command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family()).unwrap()
            .begin_render_pass(framebuffers[image_num].clone(), false,
                vec!(clear_values.into(), 1f32.into())).unwrap()
            .draw_indexed(pipeline.clone(), &DynamicState::none(), vec!(vertex_buffer.clone(), normals_buffer.clone()), 
                  v_index_buffer.clone(), (), ()).unwrap()
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

//TODO: Error handling
fn init_vulkan() -> Result<(Arc<Device>, QueuesIter, Arc<Surface<Window>>, EventsLoop), Box<Error>> {
        
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

    let events_loop = EventsLoop::new();
    let surface = WindowBuilder::new()
        .with_title("Riley's Vulkan Render Engine")
        .with_decorations(true)
        .build_vk_surface(&events_loop, instance.clone())
        .expect("Could not create window");

    let queue_family = physical_device.queue_families().find(|&q| {
        q.supports_graphics() && surface.is_supported(q).unwrap_or(false)
    }).expect("Could not find queue.");
    
    let device_extensions = DeviceExtensions {
        khr_swapchain: true,
        .. DeviceExtensions::none()
    };
    let (device, queues) = Device::new(physical_device, physical_device.supported_features(),
        &device_extensions, [(queue_family, 0.5)].iter().cloned()).expect("Could not create device");

    Ok((device, queues, surface, events_loop))
} 

fn gen_swapchain(surface: Arc<Surface<Window>>, queue: Arc<Queue>, device: Arc<Device>) 
    -> Result<(Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>), SwapchainCreationError> {
        
    let window = surface.window();

    let capabilities = surface.capabilities(device.physical_device()).unwrap();

    let usage = capabilities.supported_usage_flags;

    let alpha = capabilities.supported_composite_alpha.iter().next().unwrap();
    
    //TODO: Choose format based on our needs.
    let format = capabilities.supported_formats[0].0;
    
    //TODO: Use more layers if necessary.
    let layers = 1;

    let dimensions = get_window_dimensions(&window)?;

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
}

fn load_texture(queue: Arc<Queue>, bytes: &[u8]) ->  
    Result<(Arc<ImmutableImage<Format>>, CommandBufferExecFuture<NowFuture, AutoCommandBuffer>), ImageCreationError> {

    let image = image::load_from_memory_with_format(bytes,
        ImageFormat::PNG).expect("Could not load image").to_rgba(); 
    let w = image.width();
    let h = image.height();
    let image_data = image.into_raw().clone();

    ImmutableImage::from_iter(image_data.iter().cloned(),
        Dimensions::Dim2d { width: w, height: h },
        Format::R8G8B8A8Srgb,
        queue.clone())
} 

fn gen_framebuffers_from_window_size(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPassAbstract + Send + Sync>,
    device: Arc<Device>,
    vs: &vertex::Shader,
    fs: &frag::Shader,
    ) -> (Arc<GraphicsPipelineAbstract + Send + Sync>, Vec<Arc<FramebufferAbstract + Send + Sync>>) {

    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0
    };

    let depth_buffer = AttachmentImage::transient(device.clone(), dimensions, Format::D16Unorm)
        .expect("Failed to create depth buffer");

    let framebuffers = images.iter().map(|image| {
        Arc::new(
            Framebuffer::start(render_pass.clone())
                .add(image.clone()).unwrap()
                .add(depth_buffer.clone()).unwrap()
                .build().unwrap()
        ) as Arc<FramebufferAbstract + Send + Sync>    
    }).collect::<Vec<_>>();

    let pipeline = Arc::new(GraphicsPipeline::start()
        .vertex_input(TwoBuffersDefinition::<Vertex, Normal>::new())
        .vertex_shader(vs.main_entry_point(), ())
        .triangle_list()
        .viewports_dynamic_scissors_irrelevant(1)
        .viewports(std::iter::once(viewport))
        .fragment_shader(fs.main_entry_point(), ())
        .depth_stencil_simple_depth()
        .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
        .build(device.clone())
        .expect("Could not generate graphics pipeline"));
    
    (pipeline, framebuffers)
}

fn get_window_dimensions(window: &Window) -> Result<[u32;2], SwapchainCreationError> {
    
    //NOTE: We could set this to capabilities.current_extent.unwrap_or(DEFAULT..)
    //But since either way we want the initial dimensions to be the window dimensions
    //we just get the physical dimensions this way
    if let Some(dimensions) = window.get_inner_size() {
        let dimensions: (u32, u32) = dimensions.to_physical(window.get_hidpi_factor()).into();
        Ok([dimensions.0, dimensions.1])
    } else {
       Err(SwapchainCreationError::SurfaceLost) 
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

