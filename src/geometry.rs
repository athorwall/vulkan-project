use std::sync::Arc;

use vulkano::buffer::BufferAccess;

#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
}
impl_vertex!(Vertex, position);

pub struct Mesh {
    vertex_buffer: Arc<BufferAccesss + Send + Sync>,
}