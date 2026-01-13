use std::{cmp::Ordering, time::Duration};

use glam::IVec2;

use crate::mins_by_key::MinsBy;

/// Line segment aligned to one axis
#[derive(Debug)]
struct AxisLineSegment {
    /// Axis along which this line runs
    axis: usize,
    /// Start/end along that axis
    main_start: i32,
    main_end: i32,
    /// Position along off axis
    off: i32,
}

impl AxisLineSegment {
    fn new(axis: usize, mut start: i32, mut end: i32, off: i32) -> Self {
        if start > end {
            (end, start) = (start, end);
        }

        Self {
            axis,
            main_start: start,
            main_end: end,
            off,
        }
    }
}

/// Represents a ball movement
#[derive(Debug)]
struct RaySegment {
    origin: IVec2,
    size: IVec2,
}

impl RaySegment {
    fn new(origin: IVec2, size: IVec2) -> Self {
        assert_ne!(size.length_squared(), 0);

        Self { origin, size }
    }
}

impl RaySegment {
    fn intersects(&self, other: &AxisLineSegment) -> Option<IVec2> {
        // Distance along off axis
        let dist = other.off - self.origin[1 - other.axis];

        if self.size[1 - other.axis] == 0 {
            // Ray is parallel
            return None;
        }

        // Check if ray goes far enough
        match (dist.cmp(&0), dist.cmp(&self.size[1 - other.axis])) {
            // intersect
            (Ordering::Less, Ordering::Equal | Ordering::Greater) => (),
            // intersect
            (Ordering::Greater, Ordering::Equal | Ordering::Less) => (),
            // No intersect
            _ => return None,
        }

        // Check that ray intersects within line segment bounds
        // TODO: Precision for small moves
        let intersect = self.origin[other.axis]
            + ((dist as f32 / self.size[1 - other.axis] as f32) * self.size[other.axis] as f32)
                as i32;

        if intersect < other.main_start || other.main_end < intersect {
            // Ray misses the goalposts
            return None;
        }

        let intersect = if other.axis == 1 {
            (other.off, intersect)
        } else {
            (intersect, other.off)
        };

        Some(IVec2::from(intersect))
    }
}

pub struct GameState {
    pub arena_size: IVec2,
    pub left_paddle_pos: IVec2,
    pub right_paddle_pos: IVec2,
    pub paddle_size: u32,
    pub ball_pos: IVec2,
    pub ball_dir: IVec2,
    pub ball_speed: u32,
    pub score: (u32, u32),
    pub left_paddle_dir: i8,
    pub right_paddle_dir: i8,
    pub paddle_speed: u32,
}

impl Default for GameState {
    fn default() -> Self {
        let arena_size = (1000, 1500).into();
        let paddle_padding = 100;
        Self {
            arena_size,
            left_paddle_pos: (arena_size[0] / 2, paddle_padding).into(),
            right_paddle_pos: (arena_size[0] / 2, arena_size[1] - paddle_padding).into(),
            paddle_size: 100,
            ball_pos: arena_size / 2,
            ball_dir: (1, 1).into(),
            ball_speed: 200,
            score: (0, 0),
            left_paddle_dir: 0,
            right_paddle_dir: 0,
            paddle_speed: 50,
        }
    }
}

impl GameState {
    /// Iterate over all obstacles in the arena, and the axis along which they run
    fn obstacles(&self) -> impl Iterator<Item = AxisLineSegment> {
        [
            // Horizontal walls
            AxisLineSegment::new(1, 0, self.arena_size[1], 0),
            AxisLineSegment::new(1, 0, self.arena_size[1], self.arena_size[0]),
            // Vertical walls
            AxisLineSegment::new(0, 0, self.arena_size[0], 0),
            AxisLineSegment::new(0, 0, self.arena_size[0], self.arena_size[1]),
            // Paddles
            AxisLineSegment::new(
                0,
                self.left_paddle_pos[0] - self.paddle_size as i32 / 2,
                self.left_paddle_pos[0] + self.paddle_size as i32 / 2,
                self.left_paddle_pos[1],
            ),
            AxisLineSegment::new(
                0,
                self.right_paddle_pos[0] - self.paddle_size as i32 / 2,
                self.right_paddle_pos[0] + self.paddle_size as i32 / 2,
                self.right_paddle_pos[1],
            ),
        ]
        .into_iter()
    }

    fn move_ball(&mut self, movement: &RaySegment) -> IVec2 {
        let collisions = self
            .obstacles()
            // .inspect(|line| log::debug!("Testing line: {:?}", line))
            .filter_map(|obstacle| movement.intersects(&obstacle).map(|c| (obstacle, c)))
            // Can't collide with objects we're ontop of
            .filter(|(_, collision)| movement.origin != *collision)
            .inspect(|(obstacle, collision)| {
                log::debug!("Collided with {:?} at {:?}", obstacle, collision);
            })
            .mins_by_key(|(_, collision)| collision.distance_squared(movement.origin));

        let Some((_, collision)) = collisions.first() else {
            // Ball did not collide, so just displace and return
            return movement.origin + movement.size;
        };

        // Move ball
        let dist = collision - movement.origin;
        let mut remaining = movement.size - dist;

        // Bounce off obstacles
        for (obstacle, _) in &collisions {
            remaining[1 - obstacle.axis] *= -1;
            self.ball_dir[1 - obstacle.axis] *= -1;
        }

        if remaining.length_squared() == 0 {
            // Ball finished moving
            return *collision;
        }

        let new_movement = RaySegment::new(*collision, remaining);

        log::debug!("Recursing with {:?}", new_movement);
        self.move_ball(&new_movement)
    }

    fn reset_ball(&mut self) {
        self.ball_pos = self.arena_size / 2;
    }

    /// Advance the game
    pub fn tick(&mut self, duration: &Duration) {
        // Move the ball / bounce around
        let ball_velocity = self.ball_dir * self.ball_speed as i32;
        let ball_displacement = (ball_velocity.as_vec2() * duration.as_secs_f32()).as_ivec2();
        log::debug!(
            "Moving ball at {:?} by {:?}",
            self.ball_pos,
            ball_displacement,
        );
        self.ball_pos = self.move_ball(&RaySegment::new(self.ball_pos, ball_displacement));
        log::debug!("Ball moved to {:?}", self.ball_pos);

        // Check if anyone has scored
        if self.ball_pos[1] < self.left_paddle_pos[1] {
            // Right score
            self.score.1 += 1;
            log::info!("Right scored");
            self.reset_ball();
        } else if self.ball_pos[1] > self.right_paddle_pos[1] {
            // Left score
            self.score.0 += 1;
            log::info!("Left scored");
            self.reset_ball();
        }

        // Move paddles
        self.left_paddle_pos[0] += self.left_paddle_dir as i32 * self.paddle_speed as i32;
        self.left_paddle_pos[0] = self.left_paddle_pos[0].clamp(
            self.paddle_size as i32 / 2,
            self.arena_size[0] - self.paddle_size as i32 / 2,
        );
        self.right_paddle_pos[0] += self.right_paddle_dir as i32 * self.paddle_speed as i32;
        self.right_paddle_pos[0] = self.right_paddle_pos[0].clamp(
            self.paddle_size as i32 / 2,
            self.arena_size[0] - self.paddle_size as i32 / 2,
        );
    }
}
