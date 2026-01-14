use std::sync::{Arc, Mutex, mpsc::Sender};

use eframe::NativeOptions;
use egui::{
    Color32, InputState, Pos2, Rect, Sense, Stroke, Vec2, emath::RectTransform, pos2, vec2,
};

use crate::game_state::GameState;

pub struct Renderer {
    game: Arc<Mutex<GameState>>,
    fps: f32,
    controller: Sender<InputState>,
}

impl Renderer {
    pub fn new(game: Arc<Mutex<GameState>>, fps: f32, controller: Sender<InputState>) -> Self {
        Self {
            game,
            fps,
            controller,
        }
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

    pub fn run(self) {
        log::debug!("Starting UI");
        let options = NativeOptions::default();
        eframe::run_simple_native("Pong", options, move |ctx, _frame| {
            // Handle user input
            // For now just send entire input state so logic can be handled in the controller.
            // A bit wasteful but probably not too bad.
            let inputs = ctx.input(|input_state| input_state.clone());
            // TODO: Handle unwrap
            self.controller
                .send(inputs)
                .expect("Failed to send input to controller thread");

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
        })
        .expect("Failed to create EFrame application");

        log::debug!("Application closed");
    }
}
