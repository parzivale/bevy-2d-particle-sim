use particle_sim_rust::Simulation;

fn main() {
    Simulation::new(10, 10..20, 1..50, -1.0..1.0).simulate();
}
