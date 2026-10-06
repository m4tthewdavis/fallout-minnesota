//! Frame-time statistics for the frame-rate display, the profile log and the
//! benchmark: average frame rate, the frame time below which half, 95% and
//! 99% of frames fall, and the "1% low" frame rate that shows stutter.

use std::collections::VecDeque;

/// The last few seconds of frame times (seconds each).
#[derive(Clone, Debug, Default)]
pub struct FrameStats {
    samples: VecDeque<f32>,
    cap: usize,
}

/// A summary of a run of frames.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub frames: usize,
    pub avg_fps: f32,
    /// Frame times in milliseconds.
    pub p50_ms: f32,
    pub p95_ms: f32,
    pub p99_ms: f32,
    pub worst_ms: f32,
    /// The average frame rate of the slowest 1% of frames.
    pub low_1pct_fps: f32,
}

impl FrameStats {
    /// Keep the most recent `cap` frames.
    pub fn new(cap: usize) -> FrameStats {
        FrameStats { samples: VecDeque::with_capacity(cap), cap: cap.max(1) }
    }

    pub fn push(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        if self.samples.len() == self.cap {
            self.samples.pop_front();
        }
        self.samples.push_back(dt);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn summary(&self) -> Option<Summary> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted: Vec<f32> = self.samples.iter().copied().collect();
        sorted.sort_by(f32::total_cmp);
        let n = sorted.len();
        let pick = |p: f32| sorted[((p * n as f32).ceil() as usize).clamp(1, n) - 1] * 1000.0;
        let total: f32 = sorted.iter().sum();
        let slowest = (n / 100).max(1);
        let slow_avg = sorted[n - slowest..].iter().sum::<f32>() / slowest as f32;
        Some(Summary {
            frames: n,
            avg_fps: n as f32 / total,
            p50_ms: pick(0.5),
            p95_ms: pick(0.95),
            p99_ms: pick(0.99),
            worst_ms: sorted[n - 1] * 1000.0,
            low_1pct_fps: 1.0 / slow_avg,
        })
    }
}

impl Summary {
    /// One line for the log or the screen.
    pub fn line(&self) -> String {
        format!(
            "{:.0} fps  (1% low {:.0})  frame {:.1} ms  p95 {:.1}  p99 {:.1}  worst {:.1}",
            self.avg_fps, self.low_1pct_fps, self.p50_ms, self.p95_ms, self.p99_ms, self.worst_ms
        )
    }

    /// A CSV row: frames, avg fps, 1% low, p50, p95, p99, worst.
    pub fn csv(&self) -> String {
        format!("{},{:.1},{:.1},{:.2},{:.2},{:.2},{:.2}", self.frames, self.avg_fps, self.low_1pct_fps, self.p50_ms, self.p95_ms, self.p99_ms, self.worst_ms)
    }
}

pub const CSV_HEADER: &str = "frames,avg_fps,low_1pct_fps,p50_ms,p95_ms,p99_ms,worst_ms";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_steady_60_fps_reads_as_60() {
        let mut s = FrameStats::new(600);
        for _ in 0..600 {
            s.push(1.0 / 60.0);
        }
        let m = s.summary().unwrap();
        assert!((m.avg_fps - 60.0).abs() < 0.1);
        assert!((m.p50_ms - 16.67).abs() < 0.05 && (m.p99_ms - 16.67).abs() < 0.05);
        assert!((m.low_1pct_fps - 60.0).abs() < 0.1);
    }

    #[test]
    fn stutter_shows_in_the_tail_not_the_median() {
        let mut s = FrameStats::new(1000);
        for i in 0..1000 {
            s.push(if i % 100 == 0 { 0.1 } else { 0.01 });
        }
        let m = s.summary().unwrap();
        assert!((m.p50_ms - 10.0).abs() < 0.01, "median unaffected");
        assert!((m.worst_ms - 100.0).abs() < 0.01);
        assert!(m.low_1pct_fps < 11.0, "the 1% low catches the hitches: {}", m.low_1pct_fps);
        assert!(m.avg_fps < 100.0 && m.avg_fps > 85.0);
        assert!(m.p99_ms >= m.p95_ms && m.p95_ms >= m.p50_ms);
    }

    #[test]
    fn only_recent_frames_count_and_nonsense_is_ignored() {
        let mut s = FrameStats::new(3);
        for dt in [1.0, 1.0, 1.0, 0.01, 0.01, 0.01, f32::NAN, -1.0, 0.0] {
            s.push(dt);
        }
        assert_eq!(s.len(), 3);
        assert!((s.summary().unwrap().avg_fps - 100.0).abs() < 0.5);
        assert!(FrameStats::new(5).summary().is_none());
    }

    #[test]
    fn the_report_lines_carry_the_numbers() {
        let mut s = FrameStats::new(10);
        s.push(0.02);
        let m = s.summary().unwrap();
        assert!(m.line().starts_with("50 fps"));
        assert_eq!(m.csv().split(',').count(), CSV_HEADER.split(',').count());
    }
}
