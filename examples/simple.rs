extern crate render;

use render::render::SimpleRenderer;

fn main() {
    let mut renderer = SimpleRenderer::create();
    loop {
        renderer.do_stuff();
    }
}
