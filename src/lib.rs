mod ball;
mod particle_sim;
use bevy::prelude::*;
use std::ops::Range;

#[derive(Resource)]
pub struct Simulation {
    num_balls: u32,
    size_range: Range<u32>,
    mass_range: Range<u32>,
    velocity_range: Range<f32>,
}

impl Default for Simulation {
    fn default() -> Self {
        Self {
            num_balls: 10,
            size_range: 10..20,
            mass_range: 4..5,
            velocity_range: -1.0..1.0,
        }
    }
}

impl Simulation {
    pub fn new(
        num_balls: u32,
        size_range: Range<u32>,
        mass_range: Range<u32>,
        velocity_range: Range<f32>,
    ) -> Self {
        Self {
            num_balls,
            size_range,
            mass_range,
            velocity_range,
        }
    }

    pub fn simulate(self) {
        let mut app = App::new();
        app.insert_resource(self);
        app.add_plugins(DefaultPlugins);
        app.add_plugin(particle_sim::ParticleSim);
        app.run();
    }
}
