use std::{error::Error, io::Write};
use sunrise_protocol::{
    buf::{Codec, Reader, Writer},
    packets::play::{DebugSample, DebugSubscriptionRequest},
};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    let mut out = std::fs::File::create(&args[2])?;
    for line in std::fs::read_to_string(&args[1])?.lines() {
        let input: serde_json::Value = serde_json::from_str(line)?;
        // Sunrise encodes the values vanilla encoded; the bytes must be vanilla's.
        let mut writer = Writer::new();
        if input["packet"] == "sample" {
            let sample = serde_json::from_value(input["sample"].clone())?;
            let kind = input["kind"].as_i64().unwrap() as i32;
            DebugSample::write(&DebugSample { sample, kind }, &mut writer);
        } else {
            let subscriptions = serde_json::from_value(input["ids"].clone())?;
            DebugSubscriptionRequest::write(
                &DebugSubscriptionRequest { subscriptions },
                &mut writer,
            );
        }
        let hex: String = writer.data.iter().map(|b| format!("{b:02x}")).collect();
        writeln!(out, "{}", serde_json::json!({"wire": hex}))?;
    }
    Ok(())
}
