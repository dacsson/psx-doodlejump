#![no_std]
#![no_main]

use psx::constants::{BLACK, BLUE, GREEN, RED};
use psx::dma;
use psx::gpu::primitives::Tile8;
use psx::gpu::{Packet, Vertex, VideoMode, link_list};
use psx::hw::gpu::GP0Command;
use psx::{Framebuffer, dprintln};

const FRAME_SIZE: (usize, usize) = (320, 240);

#[unsafe(no_mangle)]
fn main() {
    // Init frame to whole screen
    let mut fb = Framebuffer::default();
    fb.set_bg_color(BLUE);

    // Init gpu dma
    let mut gpu_dma = dma::GPU::new();

    // Just a debug text
    let mut txt = fb.load_default_font().new_text_box((0, 8), (320, 240));

    // Create a moving square
    let mut square = Tile8::new();
    square.set_color(RED);
    square.set_offset(Vertex(0, 0));

    // First square is for drawing, second is for moving
    // This data is passed to gpu
    let mut data = [Packet::new(square), Packet::new(square)];
    link_list(&mut data);

    // Flag to swap draw and move squares
    let mut swapped = false;

    // Speed of the square
    let mut v_speed = 5i16;
    let mut h_speed = 5i16;

    loop {
        swapped = !swapped;

        let (a, b) = data.split_at_mut(1);
        let (draw_sq, move_sq) = if swapped { (b, a) } else { (a, b) };

        dprintln!(
            txt,
            "current position: ({}, {})",
            move_sq[0].contents.get_offset().0,
            move_sq[0].contents.get_offset().1
        );

        gpu_dma.send_list_and(draw_sq, || {
            // Move the square
            let x = move_sq[0].contents.get_offset().0;
            let y = move_sq[0].contents.get_offset().1;

            // Hit left or right wall, bounce off it
            if x + v_speed >= FRAME_SIZE.0 as i16 || x + v_speed <= 0 {
                v_speed = -v_speed;
            }

            // Hit top or bottom wall, bounce off it
            if y + h_speed >= FRAME_SIZE.1 as i16 || y + h_speed <= 0 {
                h_speed = -h_speed;
            }

            move_sq[0]
                .contents
                .set_offset(Vertex(x + v_speed, y + h_speed));
        });

        txt.reset();
        fb.draw_sync();
        fb.wait_vblank();
        fb.dma_swap(&mut gpu_dma);
    }
}
