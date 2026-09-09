#[path = "../tests/support/dedicated_journey.rs"]
mod journey;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    journey::run(
        std::path::Path::new(args.get(1).ok_or("server binary required")?),
        Some(std::path::Path::new(
            args.get(2).ok_or("capture directory required")?,
        )),
    )
}
