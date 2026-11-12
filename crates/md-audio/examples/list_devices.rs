//! Prints the available loopback (monitor) capture devices.
fn main() {
    match md_audio::list_devices() {
        Ok(devices) if devices.is_empty() => println!("no monitor sources found"),
        Ok(devices) => {
            for d in devices {
                let mark = if d.is_default { " (default)" } else { "" };
                println!("{}{mark}\n    {}", d.name, d.description);
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
