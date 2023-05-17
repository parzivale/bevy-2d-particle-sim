use std::{collections::BTreeMap, default, sync::Mutex};

use bevy::{
    ecs::query,
    prelude::*,
    tasks::{ParallelSlice, TaskPool},
};

use crate::ball::{self, Ball, Mass, Size, Velocity};

pub struct ParticleSim;

#[derive(States, Debug, Default, PartialEq, Eq, Hash, Clone, Copy)]
pub enum SimState {
    #[default]
    Setup,
    Simulate,
    Pause,
    Stop,
}

#[derive(PartialEq, Eq)]
pub enum CollisionType {
    Wall(Wall),
    Entity(Entity),
}

impl Default for CollisionType {
    fn default() -> Self {
        CollisionType::Wall(Wall::default())
    }
}

#[derive(PartialEq, Eq, Default)]
pub enum Wall {
    #[default]
    North,
    East,
    West,
    South,
}

impl Plugin for ParticleSim {
    fn build(&self, app: &mut App) {
        app.add_state::<SimState>();
        app.add_plugin(crate::ball::BallPlugin);
        app.add_startup_system(setup);
        app.add_system(collider.in_set(OnUpdate(SimState::Simulate)));
    }
}

fn collider(
    mut query: Query<(&mut Velocity, &mut Transform, &Size, &Mass, Entity), With<Ball>>,
    time: Res<Time>,
    mut camera: Query<(&Camera, &GlobalTransform)>,
) {
    let (camera, camera_transform) = camera.get_single_mut().unwrap();
    let bounds = {
        let (min, max) = camera.logical_viewport_rect().unwrap();
        (
            camera
                .viewport_to_world_2d(camera_transform, min)
                .unwrap_or_default(),
            camera
                .viewport_to_world_2d(camera_transform, max)
                .unwrap_or_default(),
        )
    };
    let colliding = Mutex::new(Vec::new());
    query
        .par_iter()
        .for_each(|(ball_vel1, ball_pos1, ball_size1, ball_mass1, entity1)| {
            query
                .par_iter()
                .for_each(|(ball_vel2, ball_pos2, ball_size2, ball_mass2, entity2)| {
                    if ball_pos1
                        .translation
                        .distance_squared(ball_pos2.translation)
                        < ((ball_size1.0 + ball_size2.0).pow(2) as f32)
                        && entity1 != entity2
                    {
                        colliding
                            .lock()
                            .unwrap()
                            .push((entity1, CollisionType::Entity(entity2)));
                    } else {
                        let (ballx, bally) = (ball_pos1.translation.x, ball_pos1.translation.y);

                        if ballx > bounds.1.x - ball_size1.0 as f32 {
                            colliding
                                .lock()
                                .unwrap()
                                .push((entity1, CollisionType::Wall(Wall::East)));
                            return;
                        }

                        if ballx < bounds.0.x + ball_size1.0 as f32 {
                            colliding
                                .lock()
                                .unwrap()
                                .push((entity1, CollisionType::Wall(Wall::West)));
                            return;
                        }

                        if bally < bounds.0.y + ball_size1.0 as f32 {
                            colliding
                                .lock()
                                .unwrap()
                                .push((entity1, CollisionType::Wall(Wall::South)));
                            return;
                        }

                        if bally > bounds.1.y - ball_size1.0 as f32 {
                            colliding
                                .lock()
                                .unwrap()
                                .push((entity1, CollisionType::Wall(Wall::North)));
                            return;
                        }
                    }
                })
        });

    let pool = TaskPool::new();
    let query = Mutex::new(query);
    colliding.lock().unwrap().dedup_by(|a, b| {
        a.0 == b.0
            || a.0
                == match b.1 {
                    CollisionType::Entity(entity) => entity,
                    _ => b.0,
                }
                && match a.1 {
                    CollisionType::Entity(entity) => entity,
                    _ => a.0,
                } == b.0
            || a.1 == b.1
    });
    colliding
        .lock()
        .unwrap()
        .par_splat_map(&pool, None, |chunk| {
            for pair in chunk {
                let entity1 = pair.0;
                match &pair.1 {
                    CollisionType::Wall(wall) => match wall {
                        Wall::North => {
                            let mut query = query.lock().unwrap();
                            let vel = &mut query.get_component_mut::<Velocity>(entity1).unwrap().0;
                            *vel = Vec2::new(vel.x, -vel.y.abs());
                        }
                        Wall::East => {
                            let mut query = query.lock().unwrap();
                            let vel = &mut query.get_component_mut::<Velocity>(entity1).unwrap().0;
                            *vel = Vec2::new(-vel.x.abs(), vel.y);
                        }
                        Wall::West => {
                            let mut query = query.lock().unwrap();
                            let vel = &mut query.get_component_mut::<Velocity>(entity1).unwrap().0;
                            *vel = Vec2::new(vel.x.abs(), vel.y);
                        }
                        Wall::South => {
                            let mut query = query.lock().unwrap();
                            let vel = &mut query.get_component_mut::<Velocity>(entity1).unwrap().0;

                            *vel = Vec2::new(vel.x, vel.y.abs());
                        }
                    },
                    CollisionType::Entity(entity2) => {
                        let mut query = query.lock().unwrap();

                        let size1 = query.get_component::<Size>(entity1).unwrap().0 as f32;
                        let size2 =
                            query.get_component::<Size>(entity2.to_owned()).unwrap().0 as f32;

                        let mass1 = query.get_component::<Mass>(entity1).unwrap().0 as f32;
                        let mass2 =
                            query.get_component::<Mass>(entity2.to_owned()).unwrap().0 as f32;

                        let position1 = query
                            .get_component::<Transform>(entity1)
                            .unwrap()
                            .translation;
                        let position2 = query
                            .get_component::<Transform>(entity2.to_owned())
                            .unwrap()
                            .translation;

                        let velocity1 = query.get_component::<Velocity>(entity1).unwrap().0;
                        let velocity2 = query
                            .get_component::<Velocity>(entity2.to_owned())
                            .unwrap()
                            .0;

                        let dist_squared = position1
                            .to_owned()
                            .distance_squared(position2.to_owned())
                            .max(1.);

                        println!("{:?}", dist_squared - (size1 + size2).powi(2));

                        let mass_scalar_1 = (2. * mass2) / (mass1 + mass2);
                        let mass_scalar_2 = (2. * mass1) / (mass2 + mass1);

                        let collision_normal_1 = (position1 - position2).truncate();
                        let collision_normal_2 = (position2 - position1).truncate();

                        let velocity_projection_1 = (velocity1 - velocity2).dot(collision_normal_1);
                        let velocity_projection_2 = (velocity2 - velocity1).dot(collision_normal_2);

                        let normalized_velocity_1 = mass_scalar_1 * velocity_projection_1
                            / dist_squared
                            * collision_normal_1;
                        let normalized_velocity_2 = mass_scalar_2 * velocity_projection_2
                            / dist_squared
                            * collision_normal_2;

                        let new_velocity_1 = velocity1 - normalized_velocity_1;
                        let new_velocity_2 = velocity2 - normalized_velocity_2;

                        query.get_component_mut::<Velocity>(entity1).unwrap().0 = new_velocity_1;
                        query
                            .get_component_mut::<Velocity>(entity2.to_owned())
                            .unwrap()
                            .0 = new_velocity_2;

                        query
                            .get_component_mut::<Transform>(entity1)
                            .unwrap()
                            .translation += (collision_normal_1.normalize_or_zero()
                            * ((size1) - (collision_normal_1.length() * (size1 / (size1 + size2)))))
                            .extend(1.);
                        query
                            .get_component_mut::<Transform>(entity2.to_owned())
                            .unwrap()
                            .translation += (collision_normal_2.normalize_or_zero()
                            * ((size2) - (collision_normal_2.length() * (size2 / (size1 + size2)))))
                            .extend(1.);
                    }
                };
            }
        });

    query
        .lock()
        .unwrap()
        .par_iter_mut()
        .for_each_mut(|(ball_vel, mut ball_pos, size, _, _)| {
            ball_pos.translation = (ball_pos.translation + ball_vel.0.extend(1.)).clamp(
                (bounds.0 + Vec2::splat(size.0 as f32 - 0.1)).extend(1.),
                (bounds.1 - Vec2::splat(size.0 as f32 - 0.1)).extend(1.),
            );
        });
    /*println!(
        "{}",
        query
            .lock()
            .unwrap()
            .iter()
            .map(
                |(vel, _, _, mass, _)| ((1. / 2. * mass.0 as f32) * vel.0.x.powf(2.))
                    + ((1. / 2. * mass.0 as f32) * vel.0.y.powf(2.))
            )
            .sum::<f32>()
    );*/
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
}
