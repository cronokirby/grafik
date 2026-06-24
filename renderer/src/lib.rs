use std::{
    thread,
    time::{Duration, Instant},
};

pub const MESSAGE: &'static str = "hello from renderer!";

#[derive(Clone, Copy, Debug)]
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RGB {
    pub fn black() -> Self {
        0x00_00_00.into()
    }
}

impl From<u32> for RGB {
    fn from(value: u32) -> Self {
        let [_, r, g, b] = value.to_be_bytes();
        Self { r, g, b }
    }
}

#[derive(Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<RGB>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        let mut out = Self {
            width,
            height,
            pixels: Vec::new(),
        };
        out.fill(RGB::from(0x00_AA_AA));
        out
    }

    /// Fill this image with a given color.
    pub fn fill(&mut self, color: RGB) {
        self.pixels.clear();
        self.pixels
            .resize((self.width * self.height) as usize, color);
    }

    pub fn write_out(&self, to: &mut [u8]) {
        for (p, chunk) in self.pixels.iter().zip(to.chunks_exact_mut(4)) {
            chunk[0] = p.r;
            chunk[1] = p.g;
            chunk[2] = p.b;
            chunk[3] = 0xFF;
        }
    }
}

pub fn render_loop(mut input: triple_buffer::Input<Image>) {
    let mut r = 0u8;
    let mut g = 0u8;
    let mut b = 0u8;
    let mut cycle = 0u8;
    loop {
        let loop_start = Instant::now();
        match cycle {
            0 => r = r.wrapping_mul(2) + 1,
            5 => g = g.wrapping_mul(3) + 1,
            10 => b = b.wrapping_mul(5) + 1,
            15 => cycle = 0,
            _ => {}
        }
        cycle += 1;
        input.input_buffer_mut().fill(RGB { r, g, b });
        input.publish();
        let time_left_in_frame = Duration::from_micros(16_667).saturating_sub(loop_start.elapsed());
        thread::sleep(time_left_in_frame);
    }
}
