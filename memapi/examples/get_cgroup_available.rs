use pymemprofile_api::oom::get_cgroup_available_memory;
use std::time::Instant;

fn main() {
    let now = Instant::now();
    for _ in 1..1_000 {
        get_cgroup_available_memory();
    }
    println!("Elapsed: {} milliseconds", now.elapsed().as_millis());
}
