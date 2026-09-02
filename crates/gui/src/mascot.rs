use eframe::egui;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CatMood {
    Idle,
    Working,
    Happy,
    Sad,
}

struct Frame {
    texture: egui::TextureHandle,
}

pub struct CatMascot {
    mood: CatMood,
    time: f32,
    mood_elapsed: f32,
    idle_closed: Frame,
    idle_open: Frame,
    working: Frame,
    happy: Frame,
    sad: Frame,
}

fn load_frame(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Frame {
    let image = image::load_from_memory(bytes).expect("bundled mascot asset should decode");
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.to_rgba8();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
    let texture = ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR);
    Frame { texture }
}

impl CatMascot {
    pub fn new(ctx: &egui::Context) -> Self {
        Self {
            mood: CatMood::Idle,
            time: 0.0,
            mood_elapsed: 0.0,
            idle_closed: load_frame(ctx, "cat_idle_closed", include_bytes!("../assets/mascot/cat_idle.png")),
            idle_open: load_frame(ctx, "cat_idle_open", include_bytes!("../assets/mascot/cat_idle_open.png")),
            working: load_frame(ctx, "cat_working", include_bytes!("../assets/mascot/cat_working.png")),
            happy: load_frame(ctx, "cat_happy", include_bytes!("../assets/mascot/cat_happy.png")),
            sad: load_frame(ctx, "cat_sad", include_bytes!("../assets/mascot/cat_sad.png")),
        }
    }

    pub fn set_mood(&mut self, mood: CatMood) {
        if self.mood != mood {
            self.mood = mood;
            self.mood_elapsed = 0.0;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.time += dt;
        self.mood_elapsed += dt;
        const REACTION_DURATION: f32 = 2.5;
        if matches!(self.mood, CatMood::Happy | CatMood::Sad) && self.mood_elapsed >= REACTION_DURATION {
            self.mood = CatMood::Idle;
            self.mood_elapsed = 0.0;
        }
    }

    fn current_texture(&self) -> &egui::TextureHandle {
        match self.mood {
            CatMood::Idle => {
                // Blink every ~3.2s: eyes closed for a short beat, open otherwise.
                let phase = self.time % 3.2;
                if (3.0..3.15).contains(&phase) {
                    &self.idle_closed.texture
                } else {
                    &self.idle_open.texture
                }
            }
            CatMood::Working => &self.working.texture,
            CatMood::Happy => &self.happy.texture,
            CatMood::Sad => &self.sad.texture,
        }
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        // Gentle bob, more energetic while working, a little hop when happy.
        let bob = match self.mood {
            CatMood::Idle => (self.time * 2.0).sin() * 1.5,
            CatMood::Working => (self.time * 6.0).sin() * 2.5,
            CatMood::Happy => (self.time * 10.0).sin().abs() * 4.0,
            CatMood::Sad => -1.5,
        };

        let texture = self.current_texture();
        let display_size = egui::vec2(44.0, 35.0);
        ui.add_space(bob.max(0.0));
        ui.image((texture.id(), display_size));
        ui.add_space((-bob).max(0.0));
    }
}