use super::perf_remote::Served;
use crate::game::client_side::profile_capture::MemorySchedule;
use crate::game::client_side::profile_capture::Profile;
use crate::game::client_side::profile_capture::ServerLink;
use std::collections::VecDeque;
use sunrise_client::client_perf::{ClientPerf, request::GpuSwitch};
use sunrise_client::perf_net::Link;
use sunrise_perf_format::v1::Trigger;

impl Profile {
    pub(in crate::game) fn new(slow_ms: f32) -> Self {
        Self {
            server: None,
            link: ServerLink::Off,
            joined: None,
            server_only: None,
            perf: ClientPerf::new(slow_ms),
            started_by: Trigger::Unspecified,
            memory: MemorySchedule::default(),
            chat_answer: None,
            jobs: VecDeque::new(),
            job: None,
            last_capture: None,
            stopped_after: None,
            progress: None,
            reply: None,
            spans_were_on: false,
            trace_file: None,
            hardware_warmed: false,
            env_tried: false,
            gpu: GpuSwitch::Off,
            served: Served::default(),
            server_link: Link::default(),
            server_reply: None,
            server_joined: false,
            server_sink: None,
            incoming: None,
        }
    }
}
