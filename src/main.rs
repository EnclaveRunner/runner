pub mod api {
    pub mod registry {
        tonic::include_proto!("registry");
    }

    pub mod task {
        tonic::include_proto!("task");
    }
}

mod config;
mod registry;

fn main() {
    let config = config::load_settings().expect("Failed to load configuration");
    println!("Loaded configuration: {:#?}", config);
}
