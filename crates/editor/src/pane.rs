//! State and pointer interaction shared by both editor viewports.

use bevy_egui::egui::{self, PointerButton, Sense};

use crate::view::{Cam, Port};
use crate::{Drag, Editor};

pub struct Pane<H, S> {
    pub stale: bool,
    pub shown: Option<S>,
    pub reframe: bool,
    pub cam: Cam,
    pub hover: Option<H>,
    pub(crate) drag: Option<Drag>,
    pub(crate) acc: f64,
}

impl<H, S> Pane<H, S> {
    pub fn new(cam: Cam) -> Self {
        Self { stale: true, shown: None, reframe: true, cam, hover: None, drag: None, acc: 0. }
    }
    pub fn invalidate(&mut self) {
        self.stale = true;
        self.shown = None;
    }
}

pub(crate) fn port(ui: &egui::Ui) -> Port {
    let rect = ui.max_rect();
    let win = ui.ctx().viewport_rect();
    Port {
        rect: [rect.left(), rect.top(), rect.right(), rect.bottom()].map(f64::from),
        window: [f64::from(win.width()), f64::from(win.height())],
    }
}

pub(crate) trait Interaction {
    type Hit: Copy;
    type Shown;
    fn pane(ed: &mut Editor) -> &mut Pane<Self::Hit, Self::Shown>;
    fn hit(&self, ed: &Editor, at: [f64; 2], px: f64) -> Option<Self::Hit>;
    fn start(&self, ed: &mut Editor, grab: [f64; 2]) -> Option<Drag>;
    fn drag(&self, ed: &mut Editor, drag: &Drag, at: [f64; 2], modifiers: (bool, bool));
    fn click(&self, ed: &mut Editor);
    fn cursor(&self, ed: &Editor) -> egui::CursorIcon;
}

pub(crate) fn interact<I: Interaction>(ui: &mut egui::Ui, ed: &mut Editor, interaction: I) {
    let resp = ui.allocate_rect(ui.max_rect(), Sense::click_and_drag());
    let port = ed.port;
    let to_stage = |ed: &mut Editor, p: egui::Pos2| I::pane(ed).cam.to_stage(&port, [f64::from(p.x), f64::from(p.y)]);
    let px = 1. / I::pane(ed).cam.zoom;
    let modifiers = ui.input(|i| (i.modifiers.shift, i.modifiers.alt));
    let mouse = resp.hover_pos().or(resp.interact_pointer_pos());
    if I::pane(ed).drag.is_none() {
        let hover = mouse.filter(|_| resp.hovered()).and_then(|m| {
            let at = to_stage(ed, m);
            interaction.hit(ed, at, px)
        });
        I::pane(ed).hover = hover;
    }
    if resp.hovered() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let k = f64::from(pinch) * (f64::from(scroll) * 0.0025).exp();
        if (k - 1.).abs() > 1e-4
            && let Some(m) = mouse
        {
            I::pane(ed).cam.zoom_at(&port, [f64::from(m.x), f64::from(m.y)], k);
        }
    }
    if resp.drag_started_by(PointerButton::Secondary) || resp.drag_started_by(PointerButton::Middle) {
        I::pane(ed).drag = Some(Drag::Pan);
    } else if resp.drag_started_by(PointerButton::Primary) {
        let grab = resp.interact_pointer_pos().map(|p| to_stage(ed, p)).unwrap_or_default();
        I::pane(ed).drag = interaction.start(ed, grab);
    }
    if resp.dragged()
        && let Some(drag) = I::pane(ed).drag.take()
    {
        if matches!(drag, Drag::Pan) {
            let d = resp.drag_delta();
            let cam = &mut I::pane(ed).cam;
            cam.center[0] -= f64::from(d.x) * px;
            cam.center[1] -= f64::from(d.y) * px;
        } else if let Some(at) = resp.interact_pointer_pos().map(|p| to_stage(ed, p)) {
            interaction.drag(ed, &drag, at, modifiers);
        }
        I::pane(ed).drag = Some(drag);
    }
    if resp.drag_stopped() {
        I::pane(ed).drag = None;
    }
    if resp.clicked() {
        interaction.click(ed);
    }
    if resp.double_clicked() && I::pane(ed).hover.is_none() {
        I::pane(ed).reframe = true;
    }
    if resp.hovered() || I::pane(ed).drag.is_some() {
        ui.ctx().set_cursor_icon(interaction.cursor(ed));
    }
}

pub(crate) fn sync_scene(scene: &mut render::Scene, cam: Cam, port: &Port) {
    scene.debug = false;
    scene.bottom_safe_area = 0.;
    scene.fixed_view = Some(cam.window_view(port));
}
