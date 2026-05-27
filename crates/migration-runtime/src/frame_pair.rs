use parking_lot::{Condvar, Mutex};
use std::sync::Arc;

#[derive(Debug)]
pub struct FramePair {
    pub frame: Mutex<Option<gst_video::VideoFrame<gst_video::video_frame::Writable>>>,
    pub cond: Condvar,
}

impl FramePair {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            frame: Mutex::new(None),
            cond: Condvar::new(),
        })
    }
}
