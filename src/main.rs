#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

use core::arch::asm;

use alloc::string::String;
use psx::constants::{BLACK, BLUE, GREEN, RED};
use psx::dma;
use psx::gpu::primitives::Tile8;
use psx::gpu::{Packet, Vertex, VideoMode, link_list};
use psx::hw::gpu::GP0Command;
use psx::sys::event::{Event, Poll};
use psx::sys::gamepad::Gamepad;
use psx::sys::kernel::{psx_enable_timer_irq, psx_get_timer};
use psx::{Framebuffer, dprintln};

const FRAME_SIZE: (i16, i16) = (320, 240);
const DEBOUNCE: u8 = 1; // Number of VBlank waits (i.e. loops) for button debounce
const JUMP_HEIGHT: i16 = 70;
const Y_SPEED_MULTIPLIER: i16 = 2;

psx::sys_heap!(500 KB);

fn get_timer(t: u32) -> u32 {
    // t is one of: 0, 1, 2
    let time: u32;
    unsafe {
        psx_get_timer(t);
        asm!(
            // Move the value from the v0 (r2) register into the output variable
            "move {0}, $v0",
            out(reg) time,
            options(nomem, nostack, preserves_flags)
        );
    }
    time
}

#[unsafe(no_mangle)]
fn main() {
    // We don't use Framebuffer's functiion to wait for VBlank, as that relies on
    // the raw IRQ register (which is impossible to use alongside the BIOS'
    // gamepad handler without taking over the kernel). So we register a polling
    // BIOS event on the VBlank IRQ.
    let vblank_event = Event::<Poll>::new(0xF2000003, 0x0002).unwrap();

    // Init frame to whole screen
    let mut fb = Framebuffer::default();
    fb.set_bg_color(BLUE);

    // Init gpu dma
    let mut gpu_dma = dma::GPU::new();

    // Just a debug text
    let mut txt = fb.load_default_font().new_text_box((0, 8), FRAME_SIZE);

    // Create a moving square
    let center_pos = ((FRAME_SIZE.0 / 2) as i16, (FRAME_SIZE.1 as i16) - 8);
    let mut square = Tile8::new();
    square.set_color(RED);
    square.set_offset(Vertex(center_pos.0, center_pos.1));

    // First square is for drawing, second is for moving
    // This data is passed to gpu
    let mut data = [Packet::new(square), Packet::new(square)];
    link_list(&mut data);

    // Flag to swap draw and move squares
    let mut swapped = false;

    // Speed of the square
    let mut v_speed = 5i16;
    let mut h_speed = 5i16;

    let mut gamepad = Gamepad::new();

    let mut debounce: u8 = 0; // debounce counter for gamepad

    let mut in_jump = false;
    let mut going_up = false;
    let mut going_down = false;

    let mut y_before_jump = square.get_offset().1;

    loop {
        let mut but_str = String::new();

        let (a, b) = data.split_at_mut(1);
        let (draw_sq, display_sq) = if swapped { (b, a) } else { (a, b) };

        gpu_dma.send_list_and(display_sq, || {
            // Move the square
            let x = display_sq[0].contents.get_offset().0;
            let y = display_sq[0].contents.get_offset().1;

            let mut next_x = x;
            let next_y;

            // Hit left or right wall, bounce off it
            if x + v_speed >= (FRAME_SIZE.0 - 8) || x + v_speed <= 8 {
                h_speed = -h_speed;
            }

            // Player controls the left-right movement
            if debounce == 0 {
                for button in gamepad.poll_p1() {
                    match button {
                        psx::sys::gamepad::Button::Left => {
                            but_str += " L ";
                            next_x = x - h_speed;
                            debounce = DEBOUNCE;
                        }
                        psx::sys::gamepad::Button::Right => {
                            but_str += " R ";
                            next_x = x + h_speed;
                            debounce = DEBOUNCE;
                        }
                        _ => {}
                    };
                }
            } else {
                debounce -= 1;
            }

            if in_jump {
                let current_height = y_before_jump - y;
                let distance_to_peak = JUMP_HEIGHT - current_height;

                if current_height < JUMP_HEIGHT && !going_down {
                    // Jumping up - slower when closer to peak
                    let v_speed_magnitude =
                        ((distance_to_peak * Y_SPEED_MULTIPLIER * 5) / JUMP_HEIGHT).max(1);
                    v_speed = -v_speed_magnitude;
                    going_up = true;
                    going_down = false;
                } else if current_height >= JUMP_HEIGHT {
                    // Jumping down - slow at peak, fast at bottom
                    let v_speed_magnitude =
                        ((current_height * Y_SPEED_MULTIPLIER) / JUMP_HEIGHT).max(1);
                    v_speed = v_speed_magnitude;
                    going_up = false;
                    going_down = true;
                } else if current_height == 0 {
                    // Landed
                    in_jump = false;
                    going_up = false;
                    going_down = false;
                    v_speed = 0;
                }
                next_y = y + v_speed;
            } else {
                y_before_jump = y;
                // Init jump
                next_y = y - 1;
                in_jump = true;
                going_up = true;
                going_down = false;
            }

            draw_sq[0].contents.set_offset(Vertex(next_x, next_y));
        });

        dprintln!(
            txt,
            "current position: ({}, {})\nbuttons: {}\ny_before_jump: {}\n",
            display_sq[0].contents.get_offset().0,
            display_sq[0].contents.get_offset().1,
            but_str,
            y_before_jump
        );

        txt.reset();
        fb.draw_sync();
        vblank_event.wait();
        fb.dma_swap(&mut gpu_dma);

        swapped = !swapped;
    }
}
