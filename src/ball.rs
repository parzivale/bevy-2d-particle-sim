use std::sync::{
    atomic::{AtomicBool, AtomicI32},
    Mutex,
};

use crate::{
    particle_sim::{SimState},
    Simulation,
};
use bevy::{prelude::*, sprite::MaterialMesh2dBundle, utils::HashMap};
use rand::prelude::*;

#[derive(Component, Default)]
pub struct Ball;

#[derive(Component, Default)]
pub struct Velocity(pub Vec2);

#[derive(Component, Default)]
pub struct Size(pub u32);

#[derive(Component, Default)]
pub struct Mass(pub u32);

#[derive(Bundle, Default)]
pub struct BallBundle {
    #[bundle]
    model: MaterialMesh2dBundle<ColorMaterial>,
    ball: Ball,
    mass: Mass,
    size: Size,
    vel: Velocity,
}

pub struct BallPlugin;

impl Plugin for BallPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            (
                create_balls,
                apply_system_buffers,
                space_balls,
                apply_system_buffers,
                pack_balls,
            )
                .chain()
                .in_schedule(OnEnter(SimState::Setup)),
        );
    }
}

fn create_balls(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    sim_params: Res<Simulation>,
) {
    for _ in 0..sim_params.num_balls {
        let size = thread_rng().gen_range(sim_params.size_range.clone());
        let mass = thread_rng().gen_range(sim_params.mass_range.clone());
        let vel = Vec2::new(
            thread_rng().gen_range(sim_params.velocity_range.clone()),
            thread_rng().gen_range(sim_params.velocity_range.clone()),
        );
        commands.spawn(BallBundle {
            model: MaterialMesh2dBundle {
                mesh: meshes
                    .add(Mesh::from(shape::Circle::new(size as f32)))
                    .into(),
                material: materials.add(ColorMaterial::from(Color::Rgba {
                    red: mass as f32,
                    green: mass as f32,
                    blue: mass as f32,
                    alpha: 1.,
                })),
                ..Default::default()
            },
            size: Size(size),
            mass: Mass(mass),
            vel: Velocity(vel),
            ..Default::default()
        });
    }
}

fn space_balls(
    mut query: Query<(&mut Transform, &Size, Entity), With<Ball>>,
    mut camera: Query<(&Camera, &GlobalTransform)>,
) {
    let (camera, camera_transform) = camera.get_single_mut().unwrap();
    let bounds = {
        let (min, max) = camera.logical_viewport_rect().unwrap();
        (
            camera.viewport_to_world_2d(camera_transform, min).unwrap(),
            camera.viewport_to_world_2d(camera_transform, max).unwrap(),
        )
    };
    for (mut transform, size, _) in query.iter_mut() {
        transform.translation = Vec3 {
            x: thread_rng().gen_range((bounds.0.x + size.0 as f32)..(bounds.1.x - size.0 as f32)),
            y: thread_rng().gen_range((bounds.0.y + size.0 as f32)..(bounds.1.y - size.0 as f32)),
            ..Default::default()
        };
    }
}

fn pack_balls(
    commands: ParallelCommands,
    mut query: Query<(&mut Transform, &Size, Entity), With<Ball>>,
    mut camera: Query<(&Camera, &GlobalTransform)>,
    time: Res<Time>,
    mut state: ResMut<NextState<SimState>>,
) {
    let (camera, camera_transform) = camera.get_single_mut().unwrap();
    let bounds = {
        let (min, max) = camera.logical_viewport_rect().unwrap();
        (
            camera.viewport_to_world_2d(camera_transform, min).unwrap(),
            camera.viewport_to_world_2d(camera_transform, max).unwrap(),
        )
    };
    println!("{:?}", bounds);
    let mut timer = Timer::from_seconds(0.1, TimerMode::Once);
    let touching: Mutex<HashMap<Entity, Vec3>> = Mutex::new(HashMap::new());
    let spaced = AtomicBool::new(false);

    while !spaced.load(std::sync::atomic::Ordering::Relaxed) && !timer.finished() {
        for (entity, vel) in touching.lock().unwrap().drain() {
            let size = query.get_component::<Size>(entity).unwrap().0.to_owned();
            let translate = &mut query
                .get_component_mut::<Transform>(entity)
                .unwrap()
                .translation;

            *translate = (*translate + vel).clamp(
                bounds.0.extend(1.) + Vec2::splat(size as f32).extend(1.),
                bounds.1.extend(1.) - Vec2::splat(size as f32).extend(1.),
            );
        }
        spaced.store(true, std::sync::atomic::Ordering::Relaxed);
        timer.tick(time.raw_delta());

        query.par_iter().for_each(|(ball_pos1, size1, entity1)| {
            query.par_iter().for_each(|(ball_pos2, size2, entity2)| {
                if ball_pos1
                    .translation
                    .distance_squared(ball_pos2.translation)
                    < ((size1.0 + size2.0).pow(2) as f32)
                    && entity1 != entity2
                {
                    spaced.store(true, std::sync::atomic::Ordering::Relaxed);
                    *touching
                        .lock()
                        .unwrap()
                        .entry(entity1)
                        .or_insert(Vec3::new(0., 0., 0.)) +=
                        (ball_pos2.translation - ball_pos1.translation).normalize_or_zero()
                            * (1. / ball_pos1.translation.length_squared()
                                * (ball_pos2
                                    .translation
                                    .distance_squared(ball_pos1.translation))
                                .max(0.01));
                    touching.lock().unwrap().entry(entity1).and_modify(|v| {
                        *v = Vec3::new(
                            thread_rng().gen_range(-1. ..1.),
                            thread_rng().gen_range(-1. ..1.),
                            1.,
                        ) + v.clamp_length_max(10000.)
                    });
                }
            });
        });
    }

    let spaced = AtomicBool::new(false);
    let cleared_entites = Mutex::new(Vec::new());
    let num_cleared = AtomicI32::new(0);
    while !spaced.load(std::sync::atomic::Ordering::Relaxed) {
        spaced.store(true, std::sync::atomic::Ordering::Relaxed);
        query.par_iter().for_each(|(ball_pos1, size1, entity1)| {
            query.par_iter().for_each(|(ball_pos2, size2, entity2)| {
                if ball_pos1
                    .translation
                    .distance_squared(ball_pos2.translation)
                    < ((size1.0 + size2.0).pow(2) as f32)
                    && entity1 != entity2
                    && !cleared_entites.lock().unwrap().contains(&entity1)
                    && !cleared_entites.lock().unwrap().contains(&entity2)
                {
                    num_cleared.fetch_add(10, std::sync::atomic::Ordering::Relaxed);
                    /*println!(
                        "deleting entity {:?}, {:?} entity to be cleared at position {:?}",
                        entity2, num_cleared, ball_pos2.translation
                    );*/
                    commands.command_scope(|mut c| c.entity(entity2).despawn_recursive());
                    cleared_entites.lock().unwrap().push(entity2);
                    spaced.store(false, std::sync::atomic::Ordering::Relaxed);
                }
            });
        });
    }
    state.set(SimState::Simulate)
}
