use std::sync::{Arc, Mutex};

use eframe::NativeOptions;
use egui::{
    Color32, InputState, Pos2, Rect, Sense, Stroke, Vec2, emath::RectTransform, pos2, vec2,
};
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::game_state::GameState;

pub struct Renderer {
    game: Arc<Mutex<GameState>>,
    fps: f32,
    controller: UnboundedSender<InputState>,
    cancel: CancellationToken,
}

impl eframe::App for Renderer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.cancel.is_cancelled() {
            log::debug!("Cancelling render thread");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Handle user input
        // For now just send entire input state so logic can be handled in the controller.
        // A bit wasteful but probably not too bad.
        let inputs = ctx.input(|input_state| input_state.clone());

        if let Err(e) = self.controller.send(inputs) {
            log::error!("Error sending controller command: {:?}", e);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Draw UI
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::canvas(ui.style()).show(ui, |ui| {
                let (resp, painter) =
                    ui.allocate_painter(ui.available_size_before_wrap(), Sense::empty());

                let arena_size = self.game.lock().unwrap().fixed.arena_size;
                let arena_size = vec2(arena_size.1 as f32, arena_size.0 as f32);

                // Transform from game space to screen space
                let to_screen = RectTransform::from_to(
                    Rect::from_min_size(Pos2::ZERO, arena_size),
                    // Pad in from the edges a bit
                    // Y points up in game space vs down in draw space
                    resp.rect.shrink(50.).scale_from_center2(Vec2::X - Vec2::Y),
                );

                // Gather game state lines to draw
                let mut lines = self.game_lines();

                // Convert to screen space
                lines.iter_mut().for_each(|line| {
                    line.iter_mut().for_each(|point| {
                        *point = to_screen.transform_pos(*point);
                    });
                });

                lines.into_iter().for_each(|line| {
                    painter.line(
                        line,
                        Stroke {
                            color: Color32::WHITE,
                            width: 5.,
                        },
                    );
                });

                // Draw ball
                let ball_pos = to_screen.transform_pos(self.ball_pos());
                painter.circle_filled(ball_pos, 3., Color32::WHITE);
            });
        });

        // Make sure we keep rendering frames even when the user is idle
        ctx.request_repaint_after_secs(1. / self.fps);
    }
}

impl Renderer {
    pub fn new(
        game: Arc<Mutex<GameState>>,
        fps: f32,
        controller: UnboundedSender<InputState>,
    ) -> (Self, CancellationToken) {
        let cancel = CancellationToken::new();
        let renderer = Self {
            game,
            fps,
            controller,
            cancel: cancel.clone(),
        };

        (renderer, cancel)
    }

    // Lines in game world space
    fn game_lines(&self) -> Vec<Vec<Pos2>> {
        let g = self.game.lock().unwrap();

        // Walls
        let walls = vec![
            pos2(0., 0.),
            pos2(0., g.fixed.arena_size.0 as f32),
            pos2(g.fixed.arena_size.1 as f32, g.fixed.arena_size.0 as f32),
            pos2(g.fixed.arena_size.1 as f32, 0.),
            // Wrap around to start
            pos2(0., 0.),
        ];

        // Paddles
        let left = vec![
            pos2(
                g.fixed.left_paddle_x as f32,
                g.dynamic.left_paddle_y as f32 - g.fixed.paddle_height as f32 / 2.,
            ),
            pos2(
                g.fixed.left_paddle_x as f32,
                g.dynamic.left_paddle_y as f32 + g.fixed.paddle_height as f32 / 2.,
            ),
        ];
        let right = vec![
            pos2(
                g.fixed.right_paddle_x as f32,
                g.dynamic.right_paddle_y as f32 - g.fixed.paddle_height as f32 / 2.,
            ),
            pos2(
                g.fixed.right_paddle_x as f32,
                g.dynamic.right_paddle_y as f32 + g.fixed.paddle_height as f32 / 2.,
            ),
        ];

        vec![walls, left, right]
    }

    // Lines in game world space
    fn ball_pos(&self) -> Pos2 {
        let g = self.game.lock().unwrap();

        pos2(
            g.dynamic.ball_position.1 as f32,
            g.dynamic.ball_position.0 as f32,
        )
    }

    pub fn run(self) -> Result<(), eframe::Error> {
        log::debug!("Starting UI");
        let options = NativeOptions::default();
        // TODO: Propagate errors back through channel
        eframe::run_native("Pong", options, Box::new(|_cc| Ok(Box::new(self))))?;

        log::debug!("Application closed");

        Ok(())
    }
}
