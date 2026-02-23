pub mod task {
    include!(concat!(env!("OUT_DIR"), "/task.rs"));
}

pub mod registry {
    include!(concat!(env!("OUT_DIR"), "/registry.rs"));
}

use task::Task;

fn main() {
    println!("Hello, world!");
}
