use eframe::egui;
use winit;

/// Moods the app can request.  Each maps to a `cat.play(action)` JS call.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CatMood {
    Idle,
    Working,
    Happy,
    Sad,
}

impl CatMood {
    /// The JS action string passed to `cat.play(…)`.
    fn action(self) -> &'static str {
        match self {
            CatMood::Idle => "idle",
            CatMood::Working => "working",
            CatMood::Happy => "happy",
            CatMood::Sad => "cry",
        }
    }
}

// ─── HTML page ──────────────────────────────────────────────────────────────
//
// The entire mascot lives here: SVG + CSS animations + JS cat.play() API.
// The page background is transparent so the WebView blends with egui's panel.

const MASCOT_HTML: &str = r##"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8"/>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html, body {
    width: 100%; height: 100%;
        background: rgb(17, 20, 26);
    overflow: hidden;
    display: flex; align-items: center; justify-content: center;
  }
</style>
</head>
<body>

<svg id="cat" class="cat-svg" xmlns="http://www.w3.org/2000/svg"
     viewBox="0 0 320 380" width="100%" height="100%">
  <style>
    .cat { transform-origin: 160px 220px; animation: bob 2.2s ease-in-out infinite; }
    .tail { transform-origin: 232px 305px; animation: tail 1.6s ease-in-out infinite; }
    .ear  { transform-origin: center; animation: ear 3.4s ease-in-out infinite; }
    .ear.r { animation-delay: .3s; }
    .eyelid { transform-box: fill-box; transform-origin: 50% 0%; animation: blink 5s infinite; }

    @keyframes bob   { 0%,100% { transform: translateY(0);    } 50% { transform: translateY(-3px); } }
    @keyframes tail  { 0%,100% { transform: rotate(0deg);     } 50% { transform: rotate(6deg);     } }
    @keyframes ear   { 0%,85%,100% { transform: rotate(0);    } 90% { transform: rotate(-5deg);    } 95% { transform: rotate(2deg); } }
    @keyframes blink { 0%,44%,48%,100% { transform: scaleY(0); } 38% { transform: scaleY(1); } }

    /* ── cry ── */
    #cat-character.crying { animation: cry-body 0.18s ease-in-out infinite; }
    #cat-character.crying #ear-left   { animation: cry-ear-left  1s   ease-in-out infinite; }
    #cat-character.crying #ear-right  { animation: cry-ear-right 1s   ease-in-out infinite; }
    #cat-character.crying #tail       { animation: cry-tail      1.8s ease-in-out infinite; }
    #cat-character.crying #pupil-left  { animation: sad-pupil-left  0.8s ease-in-out forwards; }
    #cat-character.crying #pupil-right { animation: sad-pupil-right 0.8s ease-in-out forwards; }
    #cat-character.crying #tears       { opacity: 1; }
    #cat-character.crying #tear-left   { animation: tear-left  1.2s ease-in infinite; }
    #cat-character.crying #tear-right  { animation: tear-right 1.2s ease-in infinite; }

    @keyframes cry-body      { 0%,100% { transform: translateX(0);      } 25% { transform: translateX(-1.5px); } 75% { transform: translateX(1.5px); } }
    @keyframes cry-ear-left  { 0%,100% { transform: rotate(0deg);  } 50% { transform: rotate(5deg);  } }
    @keyframes cry-ear-right { 0%,100% { transform: rotate(0deg);  } 50% { transform: rotate(-5deg); } }
    @keyframes cry-tail      { 0%,100% { transform: rotate(0deg);  } 50% { transform: rotate(-5deg); } }
    @keyframes sad-pupil-left  { from { transform: translate(0,0); } to { transform: translate(0,-4px); } }
    @keyframes sad-pupil-right { from { transform: translate(0,0); } to { transform: translate(0,-4px); } }
    @keyframes tear-left  { 0% { transform: translateY(0);    opacity:0; } 15% { opacity:1; } 100% { transform: translateY(55px); opacity:0; } }
    @keyframes tear-right { 0% { transform: translateY(0);    opacity:0; } 15% { opacity:1; } 100% { transform: translateY(55px); opacity:0; } }

    /* ── working ── */
    #cat-character.working { animation: work-bob 0.35s ease-in-out infinite; }
    #cat-character.working #tail { animation: work-tail 0.6s ease-in-out infinite; }
    @keyframes work-bob  { 0%,100% { transform: translateY(0);    } 50% { transform: translateY(-5px); } }
    @keyframes work-tail { 0%,100% { transform: rotate(-8deg);    } 50% { transform: rotate(8deg);     } }

    /* ── happy ── */
    #cat-character.happy { animation: happy-hop 0.4s ease-in-out infinite; }
    #cat-character.happy #eye-left circle:nth-child(1),
    #cat-character.happy #eye-right circle:nth-child(1) { display: none; }
    @keyframes happy-hop { 0%,100% { transform: translateY(0); } 50% { transform: translateY(-6px); } }

    #cat-character #ear-left, #cat-character #ear-right,
    #cat-character #tail,
    #cat-character #pupil-left, #cat-character #pupil-right,
    #cat-character #tear-left,  #cat-character #tear-right {
      transform-box: fill-box;
      transform-origin: center;
    }
  </style>

  <g id="cat-character" class="cat">
    <!-- tail -->
    <path id="tail" class="tail" d="M232 305 C280 300 295 255 278 215"
          fill="none" stroke="#f5941f" stroke-width="22" stroke-linecap="round"/>
    <circle cx="278" cy="215" r="15" fill="#ffffff" stroke="#000" stroke-width="4"/>
    <!-- body -->
    <ellipse cx="160" cy="290" rx="82" ry="68" fill="#f5941f" stroke="#000" stroke-width="4"/>
    <ellipse cx="160" cy="302" rx="46" ry="50" fill="#ffffff" stroke="#000" stroke-width="4"/>
    <!-- paws -->
    <ellipse cx="132" cy="358" rx="17" ry="10" fill="#ffffff" stroke="#000" stroke-width="4"/>
    <ellipse cx="188" cy="358" rx="17" ry="10" fill="#ffffff" stroke="#000" stroke-width="4"/>
    <!-- ears -->
    <path id="ear-left"        class="ear l"       d="M80.25 132.8 L60 20 L137.23 85 Z"   fill="#f5941f" stroke="#000" stroke-width="4" stroke-linejoin="round"/>
    <path id="ear-right"       class="ear r"       d="M239.75 132.8 L260 20 L182.77 85 Z" fill="#f5941f" stroke="#000" stroke-width="4" stroke-linejoin="round"/>
    <path id="ear-left-inner"  class="ear l inner" d="M92 118 L72 55 L118 85 Z"           fill="#f7a97a"/>
    <path id="ear-right-inner" class="ear r inner" d="M228 118 L248 55 L202 85 Z"         fill="#f7a97a"/>
    <!-- head -->
    <circle cx="160" cy="170" r="95" fill="#f5941f" stroke="#000" stroke-width="4"/>
    <!-- eyes -->
    <g id="eyes">
      <g id="eye-left">
        <circle cx="125" cy="165" r="30" fill="#ffffff" stroke="#000" stroke-width="4"/>
        <circle id="pupil-left"  cx="127" cy="167" r="19" fill="#1a6b7a"/>
        <circle cx="129" cy="169" r="10" fill="#0a0a0a"/>
        <circle cx="121" cy="159" r="4"  fill="#ffffff"/>
      </g>
      <g id="eye-right">
        <circle cx="195" cy="165" r="30" fill="#ffffff" stroke="#000" stroke-width="4"/>
        <circle id="pupil-right" cx="197" cy="167" r="19" fill="#1a6b7a"/>
        <circle cx="199" cy="169" r="10" fill="#0a0a0a"/>
        <circle cx="191" cy="159" r="4"  fill="#ffffff"/>
      </g>
    </g>
    <!-- eyelids -->
    <circle class="eyelid" cx="125" cy="165" r="31" fill="#f5941f"/>
    <circle class="eyelid" cx="195" cy="165" r="31" fill="#f5941f"/>
    <path   class="eyelid" d="M94 165 A31 31 0 0 1 156 165"  fill="none" stroke="#000" stroke-width="4"/>
    <path   class="eyelid" d="M164 165 A31 31 0 0 1 226 165" fill="none" stroke="#000" stroke-width="4"/>
    <!-- tears -->
    <g id="tears" opacity="0" pointer-events="none">
      <ellipse id="tear-left"  cx="125" cy="193" rx="5" ry="9" fill="#66c7e8"/>
      <ellipse id="tear-right" cx="195" cy="193" rx="5" ry="9" fill="#66c7e8"/>
    </g>
    <!-- nose -->
    <path d="M150 202 Q160 195 170 202 Q166 210 160 210 Q154 210 150 202 Z"
          fill="#2b2b3a" stroke="#000" stroke-width="1.5"/>
    <!-- mouth -->
    <path id="mouth"
          d="M160 210 L160 218 M160 218 Q150 218 145 210 M160 218 Q170 218 175 210"
          fill="none" stroke="#000" stroke-width="3" stroke-linecap="round"/>
    <!-- whiskers -->
    <g stroke="#000" stroke-width="3" stroke-linecap="round">
      <path d="M115 205 L65 198 M115 213 L62 215 M118 221 L68 230"/>
      <path d="M205 205 L255 198 M205 213 L258 215 M202 221 L252 230"/>
    </g>
  </g>
</svg>

<script>
const character = document.querySelector("#cat-character");

const cat = {
  play(action) {
    character.classList.remove("crying", "working", "happy");
    character
      .querySelectorAll("[style]")
      .forEach(el => el.removeAttribute("style"));
    switch (action) {
      case "cry":     character.classList.add("crying");  break;
      case "working": character.classList.add("working"); break;
      case "happy":   character.classList.add("happy");   break;
      case "idle":
      default:        break;
    }
  }
};
</script>

</body>
</html>
"##;

// ─── Display size ────────────────────────────────────────────────────────────
//
// The egui panel reserves this rect; the wry child window is pinned to the
// same pixel coordinates inside the OS window.

const MASCOT_W: f32 = 56.0;
const MASCOT_H: f32 = 56.0;

// ─── WebViewMascot ───────────────────────────────────────────────────────────

const REACTION_DURATION: f32 = 2.5;

pub struct WebViewMascot {
    /// Created lazily on the first `show()` call, once we have an HWND.
    webview: Option<wry::WebView>,
    /// Pending mood to apply as soon as the WebView is ready (or on next tick).
    pending_mood: Option<CatMood>,
    /// Current mood — tracked so we only call evaluate_script on changes.
    current_mood: CatMood,
    /// Seconds since the current transient mood was applied.
    mood_elapsed: f32,
}

impl WebViewMascot {
    pub fn new() -> Self {
        Self {
            webview: None,
            pending_mood: None,
            current_mood: CatMood::Idle,
            mood_elapsed: 0.0,
        }
    }

    pub fn set_mood(&mut self, mood: CatMood) {
        if self.current_mood != mood {
            self.current_mood = mood;
            self.mood_elapsed = 0.0;
            self.apply_mood();
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if matches!(self.current_mood, CatMood::Happy | CatMood::Sad | CatMood::Working) {
            self.mood_elapsed += dt;
            if self.mood_elapsed >= REACTION_DURATION {
                self.set_mood(CatMood::Idle);
            }
        }
    }

    fn apply_mood(&mut self) {
        if let Some(wv) = &self.webview {
            let action = self.current_mood.action();
            let _ = wv.evaluate_script(&format!("cat.play('{action}')"));
        } else {
            // Will be replayed once the WebView initialises.
            self.pending_mood = Some(self.current_mood);
        }
    }

    /// Call once per frame from inside the egui `update()`.
    ///
    /// Allocates a fixed-size rect in the current UI, then creates (or
    /// repositions) the wry child WebView to cover that same rect in OS
    /// window coordinates.
    pub fn show(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Reserve space in the egui layout.
        let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(MASCOT_W, MASCOT_H), egui::Sense::hover());

        // Convert to integer physical pixels for Win32 / WebView2.
        let pixels_per_point = ui.ctx().pixels_per_point();
        let to_phys = |v: f32| (v * pixels_per_point).round() as i32;

        let phys_x = to_phys(rect.min.x);
        let phys_y = to_phys(rect.min.y);
        let phys_w = to_phys(rect.width()).max(1) as u32;
        let phys_h = to_phys(rect.height()).max(1) as u32;

        // Grab the winit window — None in headless/test contexts.
        let Some(window) = frame.winit_window() else { return };

        if self.webview.is_none() {
            self.init_webview(window, phys_x, phys_y, phys_w, phys_h);
        } else {
            self.reposition(phys_x, phys_y, phys_w, phys_h);
        }
    }

    fn init_webview(
        &mut self,
        window: &std::sync::Arc<winit::window::Window>,
        x: i32, y: i32, w: u32, h: u32,
    ) {
        use wry::dpi::{PhysicalPosition, PhysicalSize};
        use wry::{Rect, WebViewBuilder};

        let bounds = Rect {
            position: PhysicalPosition::new(x, y).into(),
            size:     PhysicalSize::new(w, h).into(),
        };

        let result = WebViewBuilder::new()
            .with_bounds(bounds)
            .with_html(MASCOT_HTML)
            .with_transparent(true)
            .with_accept_first_mouse(true)
            // Disable the right-click context menu and dev tools in release.
            .with_devtools(cfg!(debug_assertions))
            .build_as_child(window);

        match result {
            Ok(wv) => {
                self.webview = Some(wv);
                // Replay any mood that was set before the WebView existed.
                if let Some(mood) = self.pending_mood.take() {
                    self.current_mood = mood;
                    self.apply_mood();
                }
            }
            Err(e) => {
                eprintln!("WebViewMascot: failed to create WebView: {e}");
            }
        }
    }

    fn reposition(&mut self, x: i32, y: i32, w: u32, h: u32) {
        use wry::dpi::{PhysicalPosition, PhysicalSize};
        use wry::Rect;

        if let Some(wv) = &self.webview {
            let bounds = Rect {
                position: PhysicalPosition::new(x, y).into(),
                size:     PhysicalSize::new(w, h).into(),
            };
            let _ = wv.set_bounds(bounds);
        }
    }
}
