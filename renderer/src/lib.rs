use std::{
    thread,
    time::{Duration, Instant},
};

pub const MESSAGE: &'static str = "hello from renderer!";

#[derive(Clone, Copy)]
pub struct RGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RGBA {
    pub fn black() -> Self {
        0x00_00_00_FF.into()
    }
}

impl From<u32> for RGBA {
    fn from(value: u32) -> Self {
        let [r, g, b, a] = value.to_be_bytes();
        Self { r, g, b, a }
    }
}

#[derive(Clone)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<RGBA>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        let mut out = Self {
            width,
            height,
            pixels: Vec::new(),
        };
        out.fill(RGBA::black());
        out
    }

    /// Fill this image with a given color.
    pub fn fill(&mut self, color: RGBA) {
        self.pixels.clear();
        self.pixels
            .resize((self.width * self.height) as usize, color);
    }

    pub fn write_out(&self, to: &mut [u8]) {
        for (p, chunk) in self.pixels.iter().zip(to.chunks_exact_mut(4)) {
            chunk[0] = p.r;
            chunk[1] = p.g;
            chunk[2] = p.b;
            chunk[3] = p.a;
        }
    }
}

pub fn render_loop(mut input: triple_buffer::Input<Image>) {
    let mut color = 0u32;
    loop {
        let loop_start = Instant::now();
        color += 1;
        let [a, b, g, r] = color.to_be_bytes();
        if a > 0 {
            color = 0;
        }
        input.input_buffer_mut().fill(RGBA { r, g, b, a });
        let time_left_in_frame = Duration::from_micros(16_667).saturating_sub(loop_start.elapsed());
        thread::sleep(time_left_in_frame);
    }
}
