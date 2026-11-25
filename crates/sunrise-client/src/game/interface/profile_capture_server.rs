#[allow(unused_imports)]
pub(in crate::game::server_side) use super::Game;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::integrated::{
    NO_ANSWER,
    capture::{Live, ServerSide, Target},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::memory::{MEMORY_EVERY_S, PendingMemory};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::owner_link::OwnerRequest;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::perf_remote::Served;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::profile_lines::{self as lines, Notice};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::Result;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::ChatAnswer;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::ENV_DIR;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::ENV_TRACE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::Job;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::MemorySchedule;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::Missing;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::NO_CAPTURE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::PROGRESS_EVERY;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::Profile;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::Progress;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::ServerLink;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::ServerOnly;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::StopRoute;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::TRACE_USAGE;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::Trace;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::Working;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::game_path;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::gpu_readout;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::json_of;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::parse_trace;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::profiler_words;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::read_bench;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::reply_text;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::reports_dir;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::resolve_capture;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::run_job;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::shown;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::shown_in;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::unavailable;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::client_side::profile_capture::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::collections::VecDeque;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::path::{Path, PathBuf};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::sync::{
    Arc,
    mpsc::{Receiver, Sender, TryRecvError},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::task::Poll;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::time::{Duration, Instant};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::client_perf::{
    self, ClientPerf,
    request::{
        self, Failed, GpuSwitch, Marked, Opened, ProfileSide, Request, Shared, StartOptions,
        Started, Status, Stopped, Traced,
    },
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::command::{
    Arg, CommandError, Dispatcher, PROFILER_GPU, Registries,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::perf_net::Link;
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::server_perf::{self, TickDetail, WorldCounts};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::spans::{self, Span};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_perf_format::{
    files,
    v1::{BenchResult, ClientSettings, StopReason, Trigger},
};

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
