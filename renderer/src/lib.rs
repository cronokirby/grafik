use std::{
    f32::consts::PI,
    thread,
    time::{Duration, Instant},
};

pub const MESSAGE: &'static str = "hello from renderer!";

struct V3 {
    x: f32,
    y: f32,
    z: f32,
}

impl V3 {
    pub fn dot(&self, other: &Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }
}

struct Screen {
    width: u32,
    height: u32,
    aspect: f32,
}

impl Screen {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            aspect: width as f32 / height as f32,
        }
    }

    pub fn rays(&self, fov: f32) -> impl Iterator<Item = (u32, u32, V3)> {
        let &Screen {
            width,
            height,
            aspect,
        } = self;
        (0..width).flat_map(move |i| {
            (0..height).map(move |j| {
                (
                    i,
                    j,
                    V3 {
                        x: (2.0 * (i as f32 + 0.5) / width as f32 - 1.0)
                            * aspect
                            * f32::tan(fov / 2.0),
                        y: (1.0 - 2.0 * (j as f32 + 0.5) / height as f32) * f32::tan(fov / 2.0),
                        z: -1.0,
                    },
                )
            })
        })
    }
}

struct Scene {
    pub sphere_pos: V3,
    pub sphere_radius: f32,
}

impl Scene {
    pub fn intersect(&self, ray: &V3) -> Option<f32> {
        let a = ray.dot(ray);
        let b = -2.0 * ray.dot(&self.sphere_pos);
        let c = self.sphere_pos.dot(&self.sphere_pos) - self.sphere_radius * self.sphere_radius;
        let delta = b * b - 4.0 * a * c;
        if delta < 0.0 {
            return None;
        }
        let delta_sqrt = delta.sqrt();
        let t0 = (-b + delta_sqrt) / 2.0 / a;
        let t1 = (-b - delta_sqrt) / 2.0 / a;
        Some(t0.min(t1))
    }
}

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

    pub fn set(&mut self, i: u32, j: u32, color: RGB) {
        self.pixels[j as usize * self.width as usize + i as usize] = color;
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
    let mut scene = Scene {
        sphere_pos: V3 {
            x: 0.0,
            y: 0.0,
            z: -8.0,
        },
        sphere_radius: 2.0,
    };
    let mut t = 0.0;
    loop {
        let loop_start = Instant::now();

        scene.sphere_radius = 1.0 + 0.5 * f32::sin(t);

        let img = input.input_buffer_mut();
        img.fill(RGB::black());
        let screen = Screen::new(img.width, img.height);
        for (i, j, ray) in screen.rays(27.0 / 365.0 * 2.0 * PI) {
            if let Some(dist) = scene.intersect(&ray) {
                let shade = (1.0 / (1.0 + dist) * 256.0) as u8;
                img.set(
                    i,
                    j,
                    RGB {
                        r: shade,
                        g: shade,
                        b: shade,
                    },
                );
            }
        }
        t += 0.01;

        input.publish();
        let time_left_in_frame = Duration::from_micros(16_667).saturating_sub(loop_start.elapsed());
        thread::sleep(time_left_in_frame);
    }
}
