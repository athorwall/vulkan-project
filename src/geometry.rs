use std::sync::Arc;

use vulkano::buffer::BufferAccess;

#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: [f32; 3],
}
impl_vertex!(Vertex, position);

pub struct Mesh {
    vertex_buffer: Arc<BufferAccess + Send + Sync>,
}

impl Mesh {
    pub fn new(vertex_buffer: Arc<BufferAccess + Send + Sync>) -> Mesh {
        Mesh {
            vertex_buffer
        }
    }

    pub fn vertex_buffer(&self) -> Arc<BufferAccess + Send + Sync> {
        self.vertex_buffer.clone()
    }
}