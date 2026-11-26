

#[cfg(test)]
pub(in crate::game) mod tests {
    use std::time::Duration;
    use {crate::game::launch_options::*, sunrise_gameplay_common::game::launch_options::{Options}};

    fn limit(args: &[&str]) -> Option<Duration> {
        crate::game::client_side::launch_options_client::time_limit(
            &Options::parse(args.iter().map(|arg| arg.to_string()))
                .unwrap()
                .unwrap(),
        )
    }

    /// Upstream's regression: a control-socket run must not outlive its explicit deadline.
    #[test]
    fn timeout_seconds_limits_every_run() {
        let secs = |n| Some(Duration::from_secs(n));
        assert_eq!(limit(&[]), None);
        assert_eq!(limit(&["--control-socket", "c.sock"]), None);
        assert_eq!(
            limit(&["--control-socket", "c.sock", "--timeout-seconds", "5"]),
            secs(5)
        );
        assert_eq!(limit(&["--timeout-seconds", "600"]), secs(600));
        assert_eq!(limit(&["--frames", "10"]), secs(120));
        assert_eq!(limit(&["--world-frames", "10"]), secs(120));
        assert_eq!(
            limit(&["--frames", "10", "--timeout-seconds", "30"]),
            secs(30)
        );
    }
}